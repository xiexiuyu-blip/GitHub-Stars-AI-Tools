-- Fox Stars Lab migration 015 scenario_evaluation_groups
-- Extracted best-effort from gsat-desktop 1.4.0-fox.1 (commit c352222) string table @0xee615d
-- Verify against schema/live_schema_018.sql before use.

CREATE TABLE scenario_evaluation_groups (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  name TEXT NOT NULL,
  normalized_name TEXT NOT NULL,
  problem_statement TEXT NOT NULL DEFAULT '',
  constraints TEXT NOT NULL DEFAULT '',
  manual_note TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft', 'evaluating', 'decided', 'archived')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  UNIQUE(account_id, normalized_name)
);

CREATE TABLE scenario_evaluation_candidates (
  scenario_id TEXT NOT NULL,
  account_id TEXT NOT NULL,
  tool_id TEXT NOT NULL,
  candidate_role TEXT NOT NULL DEFAULT 'alternative' CHECK(candidate_role IN ('primary', 'alternative', 'watch')),
  evaluation_status TEXT NOT NULL DEFAULT 'unreviewed' CHECK(evaluation_status IN ('unreviewed', 'researching', 'trial', 'accepted', 'rejected')),
  manual_note TEXT NOT NULL DEFAULT '',
  sort_order INTEGER NOT NULL DEFAULT 0 CHECK(sort_order >= 0),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  PRIMARY KEY(scenario_id, tool_id),
  FOREIGN KEY(scenario_id) REFERENCES scenario_evaluation_groups(id) ON DELETE CASCADE,
  FOREIGN KEY(tool_id) REFERENCES tools(id) ON DELETE CASCADE,
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE
);

CREATE TRIGGER trg_scenario_candidate_account_insert
BEFORE INSERT ON scenario_evaluation_candidates
FOR EACH ROW WHEN NOT EXISTS(
  SELECT 1 FROM scenario_evaluation_groups s WHERE s.id = NEW.scenario_id AND s.account_id = NEW.account_id
) OR NOT EXISTS(
  SELECT 1 FROM tools t WHERE t.id = NEW.tool_id AND t.account_id = NEW.account_id
)
BEGIN
  SELECT RAISE(ABORT, 'scenario candidate must share account');
END;

CREATE TRIGGER trg_scenario_candidate_account_update
BEFORE UPDATE ON scenario_evaluation_candidates
FOR EACH ROW WHEN NOT EXISTS(
  SELECT 1 FROM scenario_evaluation_groups s WHERE s.id = NEW.scenario_id AND s.account_id = NEW.account_id
) OR NOT EXISTS(
  SELECT 1 FROM tools t WHERE t.id = NEW.tool_id AND t.account_id = NEW.account_id
)
BEGIN
  SELECT RAISE(ABORT, 'scenario candidate must share account');
END;

CREATE TRIGGER trg_scenario_candidate_limit_insert
BEFORE INSERT ON scenario_evaluation_candidates
FOR EACH ROW WHEN (SELECT COUNT(*) FROM scenario_evaluation_candidates WHERE scenario_id = NEW.scenario_id) >= 8
BEGIN
  SELECT RAISE(ABORT, 'scenario supports at most 8 candidates');
END;

CREATE TRIGGER trg_scenario_candidate_limit_update
BEFORE UPDATE OF scenario_id ON scenario_evaluation_candidates
FOR EACH ROW WHEN NEW.scenario_id <> OLD.scenario_id AND (SELECT COUNT(*) FROM scenario_evaluation_candidates WHERE scenario_id = NEW.scenario_id) >= 8
BEGIN
  SELECT RAISE(ABORT, 'scenario supports at most 8 candidates');
END;

CREATE TRIGGER trg_scenario_status_candidate_gate
BEFORE UPDATE OF status ON scenario_evaluation_groups
FOR EACH ROW WHEN NEW.status IN ('evaluating', 'decided') AND (
  (SELECT COUNT(*) FROM scenario_evaluation_candidates WHERE scenario_id = NEW.id) < 2
  OR (SELECT COUNT(*) FROM scenario_evaluation_candidates WHERE scenario_id = NEW.id) > 8
)
BEGIN
  SELECT RAISE(ABORT, 'evaluating or decided scenarios require 2 to 8 candidates');
END;

CREATE TRIGGER trg_scenario_insert_status_candidate_gate
BEFORE INSERT ON scenario_evaluation_groups
FOR EACH ROW WHEN NEW.status IN ('evaluating', 'decided')
BEGIN
  SELECT RAISE(ABORT, 'evaluating or decided scenarios require 2 to 8 candidates');
END;

CREATE TRIGGER trg_scenario_candidate_delete_downgrade
AFTER DELETE ON scenario_evaluation_candidates
FOR EACH ROW
BEGIN
  UPDATE scenario_evaluation_groups
  SET status = CASE
        WHEN status IN ('evaluating', 'decided')
          AND (SELECT COUNT(*) FROM scenario_evaluation_candidates WHERE scenario_id = OLD.scenario_id) < 2
        THEN 'draft'
        ELSE status
      END,
      updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
  WHERE id = OLD.scenario_id AND account_id = OLD.account_id;
END;

CREATE INDEX idx_scenario_groups_account_status ON scenario_evaluation_groups(account_id, status, updated_at);
CREATE INDEX idx_scenario_candidates_account_scenario ON scenario_evaluation_candidates(account_id, scenario_id, sort_order);
CREATE UNIQUE INDEX idx_scenario_candidates_sort_order ON scenario_evaluation_candidates(scenario_id, sort_order);

INSERT INTO schema_migrations(version, name) VALUES('015', 'scenario_evaluation_groups');
