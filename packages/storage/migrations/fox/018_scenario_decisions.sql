-- Fox Stars Lab migration 018 scenario_decisions
-- Extracted best-effort from gsat-desktop 1.4.0-fox.1 (commit c352222) string table @0xedb1b2
-- Verify against schema/live_schema_018.sql before use.

CREATE TABLE scenario_decisions (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  scenario_id TEXT NOT NULL,
  revision INTEGER NOT NULL,
  parent_decision_id TEXT,
  status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','confirmed','superseded')),
  chosen_tool_id TEXT,
  chosen_tool_name TEXT NOT NULL DEFAULT '',
  alternative_tool_id TEXT,
  alternative_tool_name TEXT NOT NULL DEFAULT '',
  why_chosen TEXT NOT NULL DEFAULT '',
  why_not_alternative TEXT NOT NULL DEFAULT '',
  evidence_summary TEXT NOT NULL DEFAULT '',
  decided_at TEXT,
  review_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  confirmed_at TEXT,
  superseded_at TEXT,
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  FOREIGN KEY(scenario_id) REFERENCES scenario_evaluation_groups(id) ON DELETE CASCADE,
  FOREIGN KEY(parent_decision_id) REFERENCES scenario_decisions(id) ON DELETE SET NULL,
  UNIQUE(account_id, scenario_id, revision)
);

CREATE TABLE scenario_decision_evidence (
  -- Every business column is an immutable, authoritative snapshot of one
  -- current comparison cell. Decision-specific manual reasoning belongs on
  -- scenario_decisions (why_* and evidence_summary), never in this table.
  id TEXT PRIMARY KEY,
  decision_id TEXT NOT NULL,
  account_id TEXT NOT NULL,
  scenario_id TEXT NOT NULL,
  tool_id TEXT,
  tool_name TEXT NOT NULL,
  criterion_id TEXT,
  criterion_name TEXT NOT NULL,
  fact_state TEXT NOT NULL,
  value TEXT NOT NULL,
  fox_judgment TEXT NOT NULL DEFAULT '',
  evidence_note TEXT NOT NULL DEFAULT '',
  source_kind TEXT,
  source_id TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  FOREIGN KEY(decision_id) REFERENCES scenario_decisions(id) ON DELETE CASCADE,
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE
);
CREATE INDEX idx_scenario_decisions_current ON scenario_decisions(account_id, scenario_id, revision DESC);
CREATE INDEX idx_scenario_decision_evidence_decision ON scenario_decision_evidence(decision_id);

CREATE TRIGGER trg_scenario_decision_scope_insert BEFORE INSERT ON scenario_decisions
FOR EACH ROW WHEN NOT EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.account_id=NEW.account_id)
 OR EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.status='archived')
 OR (NEW.chosen_tool_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM scenario_evaluation_candidates c WHERE c.scenario_id=NEW.scenario_id AND c.account_id=NEW.account_id AND c.tool_id=NEW.chosen_tool_id))
 OR (NEW.alternative_tool_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM scenario_evaluation_candidates c WHERE c.scenario_id=NEW.scenario_id AND c.account_id=NEW.account_id AND c.tool_id=NEW.alternative_tool_id))
BEGIN SELECT RAISE(ABORT, 'decision must use an active owned scenario candidate'); END;
CREATE TRIGGER trg_scenario_decision_scope_update BEFORE UPDATE ON scenario_decisions
FOR EACH ROW WHEN EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.status='archived')
 OR NOT EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.account_id=NEW.account_id)
 OR (NEW.chosen_tool_id IS NOT OLD.chosen_tool_id AND NEW.chosen_tool_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM scenario_evaluation_candidates c WHERE c.scenario_id=NEW.scenario_id AND c.account_id=NEW.account_id AND c.tool_id=NEW.chosen_tool_id))
 OR (NEW.alternative_tool_id IS NOT OLD.alternative_tool_id AND NEW.alternative_tool_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM scenario_evaluation_candidates c WHERE c.scenario_id=NEW.scenario_id AND c.account_id=NEW.account_id AND c.tool_id=NEW.alternative_tool_id))
BEGIN SELECT RAISE(ABORT, 'decision must use an active owned scenario candidate'); END;

CREATE TRIGGER trg_scenario_decision_parent_insert BEFORE INSERT ON scenario_decisions
FOR EACH ROW WHEN
  NEW.status<>'draft'
  OR (NEW.parent_decision_id IS NULL AND EXISTS(SELECT 1 FROM scenario_decisions current WHERE current.account_id=NEW.account_id AND current.scenario_id=NEW.scenario_id AND current.status='confirmed'))
  OR (NEW.parent_decision_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM scenario_decisions current WHERE current.id=NEW.parent_decision_id AND current.account_id=NEW.account_id AND current.scenario_id=NEW.scenario_id AND current.status='confirmed'))
BEGIN SELECT RAISE(ABORT, 'decision insert must be a draft with the current confirmed parent'); END;

CREATE TRIGGER trg_scenario_decision_parent_update BEFORE UPDATE ON scenario_decisions
FOR EACH ROW WHEN NEW.status='draft' AND
  ((NEW.parent_decision_id IS NULL AND EXISTS(SELECT 1 FROM scenario_decisions current WHERE current.account_id=NEW.account_id AND current.scenario_id=NEW.scenario_id AND current.status='confirmed')) OR (NEW.parent_decision_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM scenario_decisions current WHERE current.id=NEW.parent_decision_id AND current.account_id=NEW.account_id AND current.scenario_id=NEW.scenario_id AND current.status='confirmed')))
BEGIN SELECT RAISE(ABORT, 'decision revision parent must match the current confirmed conclusion'); END;

CREATE TRIGGER trg_scenario_decision_state_update BEFORE UPDATE OF status ON scenario_decisions
FOR EACH ROW WHEN
  (OLD.status='draft' AND NEW.status NOT IN ('draft','confirmed'))
  OR (OLD.status='confirmed' AND NEW.status<>'superseded')
  OR OLD.status='superseded'
BEGIN SELECT RAISE(ABORT, 'decision status transition is not allowed'); END;

CREATE TRIGGER trg_scenario_decision_confirm_transition BEFORE UPDATE OF status ON scenario_decisions
FOR EACH ROW WHEN OLD.status='draft' AND NEW.status='confirmed'
BEGIN
  SELECT CASE WHEN NEW.id IS NOT OLD.id OR NEW.account_id IS NOT OLD.account_id OR NEW.scenario_id IS NOT OLD.scenario_id OR NEW.revision IS NOT OLD.revision OR NEW.parent_decision_id IS NOT OLD.parent_decision_id OR NEW.chosen_tool_id IS NOT OLD.chosen_tool_id OR NEW.chosen_tool_name IS NOT OLD.chosen_tool_name OR NEW.alternative_tool_id IS NOT OLD.alternative_tool_id OR NEW.alternative_tool_name IS NOT OLD.alternative_tool_name OR NEW.why_chosen IS NOT OLD.why_chosen OR NEW.why_not_alternative IS NOT OLD.why_not_alternative OR NEW.evidence_summary IS NOT OLD.evidence_summary OR NEW.decided_at IS NOT OLD.decided_at OR NEW.review_at IS NOT OLD.review_at OR NEW.created_at IS NOT OLD.created_at OR NEW.superseded_at IS NOT OLD.superseded_at THEN RAISE(ABORT, 'decision confirmation may not edit draft fields') END;
  SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM scenario_evaluation_groups scenario WHERE scenario.id=NEW.scenario_id AND scenario.account_id=NEW.account_id AND scenario.status<>'archived') THEN RAISE(ABORT, 'decision confirmation scenario is unavailable') END;
  SELECT CASE WHEN NEW.chosen_tool_id IS NULL OR NEW.chosen_tool_name='' OR NOT EXISTS(SELECT 1 FROM scenario_evaluation_candidates candidate JOIN tools tool ON tool.id=candidate.tool_id AND tool.account_id=candidate.account_id WHERE candidate.scenario_id=NEW.scenario_id AND candidate.account_id=NEW.account_id AND candidate.tool_id=NEW.chosen_tool_id AND tool.name=NEW.chosen_tool_name) THEN RAISE(ABORT, 'decision confirmation requires the current chosen tool') END;
  SELECT CASE WHEN (NEW.alternative_tool_id IS NULL AND NEW.alternative_tool_name<>'') OR (NEW.alternative_tool_id IS NOT NULL AND (NEW.alternative_tool_id=NEW.chosen_tool_id OR NOT EXISTS(SELECT 1 FROM scenario_evaluation_candidates candidate JOIN tools tool ON tool.id=candidate.tool_id AND tool.account_id=candidate.account_id WHERE candidate.scenario_id=NEW.scenario_id AND candidate.account_id=NEW.account_id AND candidate.tool_id=NEW.alternative_tool_id AND tool.name=NEW.alternative_tool_name))) THEN RAISE(ABORT, 'decision confirmation alternative is invalid') END;
  SELECT CASE WHEN trim(NEW.why_chosen)='' OR NEW.decided_at IS NULL OR NEW.decided_at NOT GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' OR NEW.confirmed_at IS NULL THEN RAISE(ABORT, 'decision confirmation requires a reason, date and confirmation time') END;
  SELECT CASE WHEN NEW.parent_decision_id IS NULL AND EXISTS(SELECT 1 FROM scenario_decisions current WHERE current.account_id=NEW.account_id AND current.scenario_id=NEW.scenario_id AND current.status='confirmed') THEN RAISE(ABORT, 'decision revision parent must match the current confirmed conclusion') END;
  SELECT CASE WHEN NEW.parent_decision_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM scenario_decisions current WHERE current.id=NEW.parent_decision_id AND current.account_id=NEW.account_id AND current.scenario_id=NEW.scenario_id AND current.status='confirmed') THEN RAISE(ABORT, 'decision revision parent must match the current confirmed conclusion') END;
  SELECT CASE WHEN EXISTS(SELECT 1 FROM scenario_decision_evidence evidence WHERE evidence.decision_id=NEW.id AND NOT EXISTS(SELECT 1 FROM scenario_evaluation_candidates candidate JOIN tools tool ON tool.id=evidence.tool_id AND tool.account_id=candidate.account_id JOIN scenario_comparison_criteria criterion ON criterion.id=evidence.criterion_id AND criterion.account_id=candidate.account_id AND criterion.scenario_id=candidate.scenario_id JOIN scenario_comparison_cells cell ON cell.account_id=candidate.account_id AND cell.scenario_id=candidate.scenario_id AND cell.tool_id=evidence.tool_id AND cell.criterion_id=evidence.criterion_id WHERE candidate.account_id=NEW.account_id AND candidate.scenario_id=NEW.scenario_id AND evidence.account_id=NEW.account_id AND evidence.scenario_id=NEW.scenario_id AND evidence.tool_name=tool.name AND evidence.criterion_name=criterion.name AND evidence.fact_state=cell.fact_state AND evidence.value=cell.value AND evidence.fox_judgment=cell.fox_judgment AND evidence.evidence_note=cell.evidence_note AND ((cell.evidence_repository_id IS NOT NULL AND evidence.source_kind='repository' AND evidence.source_id=cell.evidence_repository_id AND EXISTS(SELECT 1 FROM tool_repositories source WHERE source.tool_id=cell.tool_id AND source.account_id=cell.account_id AND source.repository_id=cell.evidence_repository_id)) OR (cell.evidence_inbox_item_id IS NOT NULL AND evidence.source_kind='inbox' AND evidence.source_id=cell.evidence_inbox_item_id AND EXISTS(SELECT 1 FROM tool_inbox_items source WHERE source.tool_id=cell.tool_id AND source.account_id=cell.account_id AND source.inbox_item_id=cell.evidence_inbox_item_id)) OR (cell.evidence_repository_id IS NULL AND cell.evidence_inbox_item_id IS NULL AND evidence.source_kind IS NULL AND evidence.source_id IS NULL)))) THEN RAISE(ABORT, 'decision confirmation evidence is stale') END;
END;

-- A confirmed child structurally authorizes replacement of its exact parent;
-- no persisted guard, nonce, or writable internal capability is involved.
CREATE TRIGGER trg_scenario_decision_supersede_parent_after_confirm AFTER UPDATE OF status ON scenario_decisions
FOR EACH ROW WHEN OLD.status='draft' AND NEW.status='confirmed' AND NEW.parent_decision_id IS NOT NULL
BEGIN
  UPDATE scenario_decisions SET status='superseded', superseded_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'), updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=NEW.parent_decision_id AND account_id=NEW.account_id AND scenario_id=NEW.scenario_id AND status='confirmed';
END;

CREATE TRIGGER trg_scenario_decision_confirmed_immutable BEFORE UPDATE ON scenario_decisions
FOR EACH ROW WHEN OLD.status='confirmed' AND NOT (NEW.status='superseded' AND NEW.id IS OLD.id AND NEW.account_id IS OLD.account_id AND NEW.scenario_id IS OLD.scenario_id AND NEW.revision IS OLD.revision AND NEW.parent_decision_id IS OLD.parent_decision_id AND NEW.chosen_tool_id IS OLD.chosen_tool_id AND NEW.chosen_tool_name IS OLD.chosen_tool_name AND NEW.alternative_tool_id IS OLD.alternative_tool_id AND NEW.alternative_tool_name IS OLD.alternative_tool_name AND NEW.why_chosen IS OLD.why_chosen AND NEW.why_not_alternative IS OLD.why_not_alternative AND NEW.evidence_summary IS OLD.evidence_summary AND NEW.decided_at IS OLD.decided_at AND NEW.review_at IS OLD.review_at AND NEW.created_at IS OLD.created_at AND NEW.confirmed_at IS OLD.confirmed_at AND NEW.superseded_at IS NOT NULL AND EXISTS(SELECT 1 FROM scenario_decisions child WHERE child.parent_decision_id=OLD.id AND child.account_id=OLD.account_id AND child.scenario_id=OLD.scenario_id AND child.status='confirmed'))
BEGIN SELECT RAISE(ABORT, 'confirmed decision is immutable; create a new revision'); END;
CREATE TRIGGER trg_scenario_decision_superseded_immutable BEFORE UPDATE ON scenario_decisions
FOR EACH ROW WHEN OLD.status='superseded'
BEGIN SELECT RAISE(ABORT, 'superseded decision is immutable'); END;
CREATE TRIGGER trg_scenario_decision_evidence_scope_insert BEFORE INSERT ON scenario_decision_evidence
FOR EACH ROW WHEN NOT EXISTS(
  SELECT 1 FROM scenario_decisions d
  JOIN scenario_evaluation_candidates candidate ON candidate.scenario_id=d.scenario_id AND candidate.account_id=d.account_id AND candidate.tool_id=NEW.tool_id
  JOIN tools tool ON tool.id=NEW.tool_id AND tool.account_id=d.account_id
  JOIN scenario_comparison_criteria criterion ON criterion.id=NEW.criterion_id AND criterion.scenario_id=d.scenario_id AND criterion.account_id=d.account_id
  JOIN scenario_comparison_cells cell ON cell.account_id=d.account_id AND cell.scenario_id=d.scenario_id AND cell.tool_id=NEW.tool_id AND cell.criterion_id=NEW.criterion_id
  WHERE d.id=NEW.decision_id AND d.account_id=NEW.account_id AND d.scenario_id=NEW.scenario_id AND d.status='draft'
    AND NEW.tool_name=tool.name AND NEW.criterion_name=criterion.name
    AND NEW.fact_state=cell.fact_state AND NEW.value=cell.value
    AND NEW.fox_judgment=cell.fox_judgment AND NEW.evidence_note=cell.evidence_note
    AND (
      (cell.evidence_repository_id IS NOT NULL AND NEW.source_kind='repository' AND NEW.source_id=cell.evidence_repository_id AND EXISTS(SELECT 1 FROM tool_repositories source WHERE source.tool_id=cell.tool_id AND source.account_id=cell.account_id AND source.repository_id=cell.evidence_repository_id))
      OR (cell.evidence_inbox_item_id IS NOT NULL AND NEW.source_kind='inbox' AND NEW.source_id=cell.evidence_inbox_item_id AND EXISTS(SELECT 1 FROM tool_inbox_items source WHERE source.tool_id=cell.tool_id AND source.account_id=cell.account_id AND source.inbox_item_id=cell.evidence_inbox_item_id))
      OR (cell.evidence_repository_id IS NULL AND cell.evidence_inbox_item_id IS NULL AND NEW.source_kind IS NULL AND NEW.source_id IS NULL)
    )
)
BEGIN SELECT RAISE(ABORT, 'decision evidence must exactly snapshot a current owned comparison cell'); END;

CREATE TRIGGER trg_scenario_decision_evidence_scope_update BEFORE UPDATE ON scenario_decision_evidence
FOR EACH ROW WHEN NOT EXISTS(
  SELECT 1 FROM scenario_decisions d
  JOIN scenario_evaluation_candidates candidate ON candidate.scenario_id=d.scenario_id AND candidate.account_id=d.account_id AND candidate.tool_id=NEW.tool_id
  JOIN tools tool ON tool.id=NEW.tool_id AND tool.account_id=d.account_id
  JOIN scenario_comparison_criteria criterion ON criterion.id=NEW.criterion_id AND criterion.scenario_id=d.scenario_id AND criterion.account_id=d.account_id
  JOIN scenario_comparison_cells cell ON cell.account_id=d.account_id AND cell.scenario_id=d.scenario_id AND cell.tool_id=NEW.tool_id AND cell.criterion_id=NEW.criterion_id
  WHERE d.id=NEW.decision_id AND d.account_id=NEW.account_id AND d.scenario_id=NEW.scenario_id AND d.status='draft'
    AND NEW.tool_name=tool.name AND NEW.criterion_name=criterion.name
    AND NEW.fact_state=cell.fact_state AND NEW.value=cell.value
    AND NEW.fox_judgment=cell.fox_judgment AND NEW.evidence_note=cell.evidence_note
    AND (
      (cell.evidence_repository_id IS NOT NULL AND NEW.source_kind='repository' AND NEW.source_id=cell.evidence_repository_id AND EXISTS(SELECT 1 FROM tool_repositories source WHERE source.tool_id=cell.tool_id AND source.account_id=cell.account_id AND source.repository_id=cell.evidence_repository_id))
      OR (cell.evidence_inbox_item_id IS NOT NULL AND NEW.source_kind='inbox' AND NEW.source_id=cell.evidence_inbox_item_id AND EXISTS(SELECT 1 FROM tool_inbox_items source WHERE source.tool_id=cell.tool_id AND source.account_id=cell.account_id AND source.inbox_item_id=cell.evidence_inbox_item_id))
      OR (cell.evidence_repository_id IS NULL AND cell.evidence_inbox_item_id IS NULL AND NEW.source_kind IS NULL AND NEW.source_id IS NULL)
    )
)
BEGIN SELECT RAISE(ABORT, 'decision evidence must exactly snapshot a current owned comparison cell'); END;

INSERT INTO schema_migrations(version, name) VALUES('018', 'scenario_decisions');
