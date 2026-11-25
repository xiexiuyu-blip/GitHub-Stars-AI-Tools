"""在临时目录用替身命令验证发布门禁，不读真实额度、不执行真实 Git 或构建。"""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


class ReleaseGateTests(unittest.TestCase):
    def run_gate(self, script, fail='', budget='{"dailyBudgetTokens":0}', quiet=False, queue_kind='file'):
        with tempfile.TemporaryDirectory(prefix='fsl-release-gate-') as directory:
            base = Path(directory)
            repo, integration, card = [base / name for name in ('repo', 'wt/_integration', 'wt/sample')]
            trace = base / 'trace'
            queue = base / 'queue.json'
            if queue_kind == 'directory':
                queue.mkdir()
            elif queue_kind == 'broken-link':
                queue.symlink_to(base / 'missing.json')
            elif queue_kind != 'missing':
                queue.write_text(budget)
            bin_dir = base / 'bin'
            bin_dir.mkdir()
            git = bin_dir / 'git'
            git.write_text('''#!/usr/bin/env bash
printf 'git %s\\n' "$*" >> "$FSL_TEST_TRACE"
if [ "$1" = rev-list ]; then echo 1; fi
exit 0
''')
            git.chmod(0o755)
            for folder in (repo, integration, card):
                (folder / 'tools/cards').mkdir(parents=True)
                runner = folder / 'tools/cards/run.sh'
                runner.write_text('''#!/usr/bin/env bash
printf 'run %s\\n' "$*" >> "$FSL_TEST_TRACE"
if [ "$1" = "$FSL_TEST_FAIL" ]; then echo 'deliberate failure' >&2; exit 37; fi
if [ "$1" = rust-test ]; then echo 'test result: ok'; fi
if [ "$FSL_TEST_QUIET" != 1 ]; then
  if [ "$1" = web-build ]; then echo 'built in 1ms'; fi
  if [ "$1" = web-test ]; then echo 'Tests 1 passed'; fi
fi
''')
                runner.chmod(0o755)
            content = (ROOT / 'tools/cards' / script).read_text()
            content = content.replace('/tmp/fsl-', str(base / 'fsl-'))
            content = content.replace('REPO="/Users/fox/WorkSpace/_infra/fox-stars-lab"', f'REPO="{repo}"')
            content = content.replace('INT="/Users/fox/WorkSpace/_infra/fox-stars-lab-wt/_integration"', f'INT="{integration}"')
            content = content.replace('WT_ROOT="/Users/fox/WorkSpace/_infra/fox-stars-lab-wt"', f'WT_ROOT="{base / "wt"}"')
            content = content.replace('DEV_QUEUE="$HOME/Library/Application Support/com.foxwork.fox-stars-lab.dev/fox-stars-lab.queue.json"', f'DEV_QUEUE="{queue}"')
            executable = base / script
            executable.write_text(content)
            env = os.environ.copy()
            env.update(PATH=str(bin_dir) + os.pathsep + env['PATH'], FSL_TEST_TRACE=str(trace), FSL_TEST_FAIL=fail, FSL_TEST_QUIET='1' if quiet else '0')
            env.pop('RESET_DEV_BUDGET', None)
            env.pop('KEEP_DEV_BUDGET', None)
            args = ['bash', str(executable)]
            if script == 'merge-card.sh':
                args += ['sample', 'test only']
            result = subprocess.run(args, env=env, capture_output=True, text=True, errors="backslashreplace", timeout=15)
            return result, trace.read_text() if trace.exists() else ''

    def test_failed_build_blocks_release_before_other_checks_or_merge(self):
        result, trace = self.run_gate('release.sh', 'web-build')
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn('run web-test', trace)
        self.assertNotIn('git merge', trace)

    def test_each_failed_check_blocks_integration_merge(self):
        for stage in ('rust-fmt', 'rust-check', 'rust-test', 'web-build', 'web-test'):
            with self.subTest(stage=stage):
                result, trace = self.run_gate('merge-card.sh', stage)
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn('git merge --no-ff', trace)

    def test_each_release_check_failure_stops_publish(self):
        for stage in ('rust-fmt', 'rust-test', 'web-test'):
            with self.subTest(stage=stage):
                result, trace = self.run_gate('release.sh', stage)
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn('git merge', trace)

    def test_success_does_not_require_grep_matching_build_or_web_test_output(self):
        for script, merge in (('release.sh', 'git merge --ff-only'), ('merge-card.sh', 'git merge --no-ff')):
            with self.subTest(script=script):
                result, trace = self.run_gate(script, quiet=True)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertIn(merge, trace)

    def test_corrupt_or_invalid_budget_is_never_treated_as_zero(self):
        for budget in ('{broken', '{}', '[]', '{"dailyBudgetTokens":null}', '{"dailyBudgetTokens":-1}', '{"dailyBudgetTokens":true}', '{"dailyBudgetTokens":"0"}'):
            with self.subTest(budget=budget):
                result, trace = self.run_gate('release.sh', budget=budget)
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn('git merge', trace)
                self.assertIn('无法确认开发版 AI 额度', result.stderr)

    def test_positive_budget_without_existing_explicit_override_blocks_release(self):
        for amount in (10000, 2 ** 100):
            with self.subTest(amount=amount):
                result, trace = self.run_gate('release.sh', budget=f'{{"dailyBudgetTokens":{amount}}}')
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn('git merge', trace)

    def test_unavailable_queue_is_never_treated_as_zero(self):
        for kind in ('missing', 'directory', 'broken-link'):
            with self.subTest(kind=kind):
                result, trace = self.run_gate('release.sh', queue_kind=kind)
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn('git merge', trace)
                self.assertIn('无法确认开发版 AI 额度', result.stderr)


if __name__ == '__main__':
    unittest.main()
