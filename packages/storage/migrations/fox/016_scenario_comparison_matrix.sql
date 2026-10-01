-- Fox Stars Lab migration 016 scenario_comparison_matrix
-- Extracted best-effort from gsat-desktop 1.4.0-fox.1 (commit c352222) string table @0xee615d
-- Verify against schema/live_schema_018.sql before use.

CREATE TABLE scenario_comparison_criteria (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  scenario_id TEXT NOT NULL,
  name TEXT NOT NULL,
  normalized_name TEXT NOT NULL,
  criterion_type TEXT NOT NULL CHECK(criterion_type IN ('boolean','rating','text')),
  description TEXT NOT NULL DEFAULT '',
  importance TEXT NOT NULL DEFAULT 'important' CHECK(importance IN ('must','important','optional')),
  sort_order INTEGER NOT NULL CHECK(sort_order >= 0),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  FOREIGN KEY(scenario_id) REFERENCES scenario_evaluation_groups(id) ON DELETE CASCADE,
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  UNIQUE(scenario_id, normalized_name),
  UNIQUE(scenario_id, sort_order)
);

CREATE TABLE scenario_comparison_cells (
  account_id TEXT NOT NULL,
  scenario_id TEXT NOT NULL,
  tool_id TEXT NOT NULL,
  criterion_id TEXT NOT NULL,
  fact_state TEXT NOT NULL DEFAULT 'unknown' CHECK(fact_state IN ('unknown','documented','observed','disputed')),
  value TEXT NOT NULL DEFAULT 'unknown',
  fox_judgment TEXT NOT NULL DEFAULT '',
  evidence_note TEXT NOT NULL DEFAULT '',
  evidence_repository_id TEXT,
  evidence_inbox_item_id TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  PRIMARY KEY(scenario_id, tool_id, criterion_id),
  FOREIGN KEY(scenario_id, tool_id) REFERENCES scenario_evaluation_candidates(scenario_id, tool_id) ON DELETE CASCADE,
  FOREIGN KEY(criterion_id) REFERENCES scenario_comparison_criteria(id) ON DELETE CASCADE,
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  CHECK((evidence_repository_id IS NULL) OR (evidence_inbox_item_id IS NULL))
);

CREATE TRIGGER trg_scenario_criterion_account_insert BEFORE INSERT ON scenario_comparison_criteria
FOR EACH ROW WHEN NOT EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.account_id=NEW.account_id)
BEGIN SELECT RAISE(ABORT, 'comparison criterion must share account'); END;
CREATE TRIGGER trg_scenario_criterion_account_update BEFORE UPDATE ON scenario_comparison_criteria
FOR EACH ROW WHEN NOT EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.account_id=NEW.account_id)
BEGIN SELECT RAISE(ABORT, 'comparison criterion must share account'); END;
CREATE TRIGGER trg_scenario_criterion_limit BEFORE INSERT ON scenario_comparison_criteria
FOR EACH ROW WHEN (SELECT COUNT(*) FROM scenario_comparison_criteria WHERE scenario_id=NEW.scenario_id)>=12
BEGIN SELECT RAISE(ABORT, 'scenario supports at most 12 comparison criteria'); END;

CREATE TRIGGER trg_scenario_criterion_archived_insert BEFORE INSERT ON scenario_comparison_criteria
FOR EACH ROW WHEN EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.status='archived')
BEGIN SELECT RAISE(ABORT, 'archived scenario is read only'); END;
CREATE TRIGGER trg_scenario_criterion_archived_update BEFORE UPDATE ON scenario_comparison_criteria
FOR EACH ROW WHEN EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.status='archived')
BEGIN SELECT RAISE(ABORT, 'archived scenario is read only'); END;
CREATE TRIGGER trg_scenario_criterion_type_with_cells BEFORE UPDATE OF criterion_type ON scenario_comparison_criteria
FOR EACH ROW WHEN NEW.criterion_type != OLD.criterion_type AND EXISTS(SELECT 1 FROM scenario_comparison_cells cell WHERE cell.criterion_id=OLD.id)
BEGIN SELECT RAISE(ABORT, 'criterion type cannot change while cells exist'); END;

CREATE TRIGGER trg_scenario_cell_scope_insert BEFORE INSERT ON scenario_comparison_cells
FOR EACH ROW WHEN NOT EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.account_id=NEW.account_id)
 OR NOT EXISTS(SELECT 1 FROM scenario_comparison_criteria c WHERE c.id=NEW.criterion_id AND c.scenario_id=NEW.scenario_id AND c.account_id=NEW.account_id)
 OR (NEW.evidence_repository_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM tool_repositories r WHERE r.tool_id=NEW.tool_id AND r.repository_id=NEW.evidence_repository_id AND r.account_id=NEW.account_id))
 OR (NEW.evidence_inbox_item_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM tool_inbox_items i WHERE i.tool_id=NEW.tool_id AND i.inbox_item_id=NEW.evidence_inbox_item_id AND i.account_id=NEW.account_id))
BEGIN SELECT RAISE(ABORT, 'comparison cell must use owned scenario criterion and tool evidence'); END;

CREATE TRIGGER trg_scenario_cell_archived_insert BEFORE INSERT ON scenario_comparison_cells
FOR EACH ROW WHEN EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.status='archived')
BEGIN SELECT RAISE(ABORT, 'archived scenario is read only'); END;
CREATE TRIGGER trg_scenario_cell_archived_update BEFORE UPDATE OF fact_state,value,fox_judgment,evidence_note,criterion_id,account_id,tool_id,scenario_id ON scenario_comparison_cells
FOR EACH ROW WHEN EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.status='archived')
BEGIN SELECT RAISE(ABORT, 'archived scenario is read only'); END;
CREATE TRIGGER trg_scenario_cell_archived_repository_evidence BEFORE UPDATE OF evidence_repository_id ON scenario_comparison_cells
FOR EACH ROW WHEN EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.status='archived')
 AND NOT (OLD.evidence_repository_id IS NOT NULL AND NEW.evidence_repository_id IS NULL AND NEW.evidence_inbox_item_id IS OLD.evidence_inbox_item_id
  AND NOT EXISTS(SELECT 1 FROM tool_repositories r WHERE r.tool_id=OLD.tool_id AND r.repository_id=OLD.evidence_repository_id AND r.account_id=OLD.account_id))
BEGIN SELECT RAISE(ABORT, 'archived scenario evidence is read only'); END;
CREATE TRIGGER trg_scenario_cell_archived_inbox_evidence BEFORE UPDATE OF evidence_inbox_item_id ON scenario_comparison_cells
FOR EACH ROW WHEN EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.status='archived')
 AND NOT (OLD.evidence_inbox_item_id IS NOT NULL AND NEW.evidence_inbox_item_id IS NULL AND NEW.evidence_repository_id IS OLD.evidence_repository_id
  AND NOT EXISTS(SELECT 1 FROM tool_inbox_items i WHERE i.tool_id=OLD.tool_id AND i.inbox_item_id=OLD.evidence_inbox_item_id AND i.account_id=OLD.account_id))
BEGIN SELECT RAISE(ABORT, 'archived scenario evidence is read only'); END;
CREATE TRIGGER trg_scenario_cell_truth_insert BEFORE INSERT ON scenario_comparison_cells
FOR EACH ROW WHEN (NEW.fact_state='unknown' AND NEW.value!='unknown') OR (NEW.fact_state!='unknown' AND (NEW.value='unknown'
 OR ((SELECT criterion_type FROM scenario_comparison_criteria WHERE id=NEW.criterion_id)='boolean' AND NEW.value NOT IN ('yes','no','partial'))
 OR ((SELECT criterion_type FROM scenario_comparison_criteria WHERE id=NEW.criterion_id)='rating' AND NEW.value NOT IN ('1','2','3','4','5'))
 OR ((SELECT criterion_type FROM scenario_comparison_criteria WHERE id=NEW.criterion_id)='text' AND (TRIM(NEW.value)='' OR NEW.value='unknown'))))
BEGIN SELECT RAISE(ABORT, 'comparison cell fact state and value conflict'); END;
CREATE TRIGGER trg_scenario_cell_truth_update BEFORE UPDATE OF fact_state,value,criterion_id ON scenario_comparison_cells
FOR EACH ROW WHEN (NEW.fact_state='unknown' AND NEW.value!='unknown') OR (NEW.fact_state!='unknown' AND (NEW.value='unknown'
 OR ((SELECT criterion_type FROM scenario_comparison_criteria WHERE id=NEW.criterion_id)='boolean' AND NEW.value NOT IN ('yes','no','partial'))
 OR ((SELECT criterion_type FROM scenario_comparison_criteria WHERE id=NEW.criterion_id)='rating' AND NEW.value NOT IN ('1','2','3','4','5'))
 OR ((SELECT criterion_type FROM scenario_comparison_criteria WHERE id=NEW.criterion_id)='text' AND (TRIM(NEW.value)='' OR NEW.value='unknown'))))
BEGIN SELECT RAISE(ABORT, 'comparison cell fact state and value conflict'); END;

CREATE TRIGGER trg_scenario_cell_clear_repository_evidence AFTER DELETE ON tool_repositories
FOR EACH ROW BEGIN UPDATE scenario_comparison_cells SET evidence_repository_id=NULL, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE tool_id=OLD.tool_id AND evidence_repository_id=OLD.repository_id AND account_id=OLD.account_id; END;
CREATE TRIGGER trg_scenario_cell_clear_inbox_evidence AFTER DELETE ON tool_inbox_items
FOR EACH ROW BEGIN UPDATE scenario_comparison_cells SET evidence_inbox_item_id=NULL, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE tool_id=OLD.tool_id AND evidence_inbox_item_id=OLD.inbox_item_id AND account_id=OLD.account_id; END;
CREATE TRIGGER trg_scenario_cell_scope_update BEFORE UPDATE ON scenario_comparison_cells
FOR EACH ROW WHEN NOT EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.account_id=NEW.account_id)
 OR NOT EXISTS(SELECT 1 FROM scenario_comparison_criteria c WHERE c.id=NEW.criterion_id AND c.scenario_id=NEW.scenario_id AND c.account_id=NEW.account_id)
 OR (NEW.evidence_repository_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM tool_repositories r WHERE r.tool_id=NEW.tool_id AND r.repository_id=NEW.evidence_repository_id AND r.account_id=NEW.account_id))
 OR (NEW.evidence_inbox_item_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM tool_inbox_items i WHERE i.tool_id=NEW.tool_id AND i.inbox_item_id=NEW.evidence_inbox_item_id AND i.account_id=NEW.account_id))
BEGIN SELECT RAISE(ABORT, 'comparison cell must use owned scenario criterion and tool evidence'); END;

CREATE INDEX idx_scenario_criteria_account ON scenario_comparison_criteria(account_id, scenario_id, sort_order);
CREATE INDEX idx_scenario_cells_account ON scenario_comparison_cells(account_id, scenario_id, tool_id);
INSERT INTO schema_migrations(version, name) VALUES('016', 'scenario_comparison_matrix');
