-- 迁移 0002：记住当前 Space、固定标签的 URL。

ALTER TABLE spaces ADD COLUMN last_active_at INTEGER;  -- 最大者为当前 Space；全为 NULL 时取 position 最小的
ALTER TABLE tabs ADD COLUMN pinned_url TEXT;           -- 固定时的 URL；关闭固定标签后回到它

-- 已有固定标签的 pinned_url 取当前 url
UPDATE tabs SET pinned_url = url WHERE pinned = 1;

-- 每个 Space 内的未归档标签按 (pinned DESC, position) 重新编号为 0..n-1。
-- 先把新编号算进临时表再 UPDATE … FROM：直接在 UPDATE 里用相关子查询数「排在前面的行」，
-- 会读到同一条语句里已经改过的行，编号可能重复。
CREATE TEMP TABLE renumbered AS
  SELECT id, ROW_NUMBER() OVER (PARTITION BY space_id ORDER BY pinned DESC, position, id) - 1 AS pos
  FROM tabs WHERE archived_at IS NULL;
UPDATE tabs SET position = renumbered.pos FROM renumbered WHERE renumbered.id = tabs.id;
DROP TABLE renumbered;
-- 已归档的标签不参与排序
UPDATE tabs SET position = -1 WHERE archived_at IS NOT NULL;
