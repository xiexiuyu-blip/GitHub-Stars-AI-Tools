//! 资料库备份与两阶段恢复。
//!
//! 这个模块不依赖 Tauri：路径都由调用方传入。前端只能把备份文件名交给
//! `request_restore`，含路径的名字会在这里被拒绝。

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// 与 `storage::SQLITE_DATABASE_FILE_NAME` 保持一致。恢复在打开数据库之前运行，不能去调用会开库的存储层。
const DATABASE_FILE_NAME: &str = "fox-stars-lab.sqlite3";
const PENDING_RESTORE_FILE: &str = "pending-restore.json";
const RESTORE_RESULT_FILE: &str = "restore-result.json";
const AUTO_BACKUP_INTERVAL_HOURS: i64 = 20;

#[cfg(test)]
thread_local! {
    static FAIL_BEFORE_PUBLISH: std::cell::Cell<bool> = std::cell::Cell::new(false);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackupKind {
    Auto,
    Manual,
    PreRestore,
    PreMigrate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub file_name: String,
    pub kind: BackupKind,
    pub created_at: String,
    pub bytes: u64,
    pub repository_count: Option<i64>,
    pub schema_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReport {
    pub ok: bool,
    pub file_name: String,
    pub backup_created_at: Option<String>,
    pub pre_restore_file_name: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupStatus {
    pub last_auto_at: Option<String>,
    pub next_auto_at: String,
    pub last_restore: Option<RestoreReport>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PendingRestore {
    file_name: String,
    requested_at: String,
    app_max_schema_version: u32,
}

pub fn create_backup(
    db_path: &Path,
    backups_dir: &Path,
    kind: BackupKind,
) -> Result<BackupInfo, String> {
    create_backup_at(db_path, backups_dir, kind, time::OffsetDateTime::now_utc())
}

fn create_backup_at(
    db_path: &Path,
    backups_dir: &Path,
    kind: BackupKind,
    now: time::OffsetDateTime,
) -> Result<BackupInfo, String> {
    let now = now.replace_nanosecond(0).unwrap_or(now);
    fs::create_dir_all(backups_dir).map_err(|error| format!("备份目录创建失败：{error}"))?;
    let file_name = reserve_backup_name(backups_dir, kind, now)?;
    let partial = backups_dir.join(format!("{file_name}.partial"));
    let destination = backups_dir.join(&file_name);
    if let Err(error) = publish_backup(db_path, &partial, &destination) {
        remove_partial(&partial);
        return Err(error);
    }
    backup_info(&destination, kind, &file_name, now)
}

pub fn list_backups(backups_dir: &Path) -> Vec<BackupInfo> {
    let entries = match fs::read_dir(backups_dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    let mut infos = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name.contains(".partial") {
            continue;
        }
        let Some((kind, from_name)) = classify_backup_name(file_name) else {
            continue;
        };
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        let created_at = match from_name {
            Some(created) => format_rfc3339(created),
            None => match metadata.modified().ok().and_then(system_time_to_rfc3339) {
                Some(created) => created,
                None => continue,
            },
        };
        let (repository_count, schema_version) = read_stats(&path);
        infos.push(BackupInfo {
            file_name: file_name.to_owned(),
            kind,
            created_at,
            bytes: metadata.len(),
            repository_count,
            schema_version,
        });
    }
    infos.sort_by(|left, right| {
        parse_rfc3339(&right.created_at)
            .cmp(&parse_rfc3339(&left.created_at))
            .then_with(|| right.file_name.cmp(&left.file_name))
    });
    infos
}

/// 返回要删除的文件名。自动备份保留最近 14 份，再加最近 3 个日历月里每月最新的一份。
/// 手动、恢复前、迁移前备份不在返回值里。月份按 UTC 计算。
pub fn plan_retention(entries: &[BackupInfo], now: time::OffsetDateTime) -> Vec<String> {
    let mut autos: Vec<&BackupInfo> = entries
        .iter()
        .filter(|entry| entry.kind == BackupKind::Auto)
        .collect();
    autos.sort_by(|left, right| backup_time_desc(left, right));

    let mut keep = HashSet::new();
    for entry in autos.iter().take(14) {
        keep.insert(entry.file_name.clone());
    }
    for entry in &autos {
        if parse_rfc3339(&entry.created_at).is_none() {
            keep.insert(entry.file_name.clone());
        }
    }
    for month in recent_month_keys(now, 3) {
        let newest = autos
            .iter()
            .copied()
            .filter(|entry| backup_month(entry) == Some(month))
            .max_by(|left, right| backup_time_asc(left, right));
        if let Some(entry) = newest {
            keep.insert(entry.file_name.clone());
        }
    }

    let mut delete = Vec::new();
    for entry in &autos {
        if !keep.contains(&entry.file_name) {
            delete.push(entry.file_name.clone());
        }
    }
    delete.sort_by(|left_name, right_name| {
        let left_time = entries
            .iter()
            .find(|entry| entry.file_name == *left_name)
            .and_then(|entry| parse_rfc3339(&entry.created_at));
        let right_time = entries
            .iter()
            .find(|entry| entry.file_name == *right_name)
            .and_then(|entry| parse_rfc3339(&entry.created_at));
        left_time.cmp(&right_time).then(left_name.cmp(right_name))
    });
    delete
}

pub fn auto_backup_due(last_auto: Option<time::OffsetDateTime>, now: time::OffsetDateTime) -> bool {
    match last_auto {
        None => true,
        Some(last) => now - last >= time::Duration::hours(AUTO_BACKUP_INTERVAL_HOURS),
    }
}

pub fn request_restore(
    data_dir: &Path,
    backups_dir: &Path,
    file_name: &str,
    app_max_schema_version: u32,
) -> Result<(), String> {
    let backup_path = located_backup(backups_dir, file_name)?;
    validate_backup_file(&backup_path, app_max_schema_version)?;
    let pending = PendingRestore {
        file_name: file_name.to_owned(),
        requested_at: format_rfc3339(time::OffsetDateTime::now_utc()),
        app_max_schema_version,
    };
    write_json(&data_dir.join(PENDING_RESTORE_FILE), &pending)?;
    Ok(())
}

/// 有 `pending-restore.json` 才做事。调用时还没有任何活动库连接。
/// 成功或失败都会清掉标记；失败时活动库文件保持原样。没有标记时返回 `None`。
pub fn apply_pending_restore(data_dir: &Path, backups_dir: &Path) -> Option<RestoreReport> {
    let marker_path = data_dir.join(PENDING_RESTORE_FILE);
    if !marker_path.is_file() {
        return None;
    }
    let pending = match read_json::<PendingRestore>(&marker_path) {
        Ok(pending) => pending,
        Err(error) => {
            return Some(finish_with_error(
                data_dir,
                "",
                None,
                format!("待恢复标记读不出来：{error}"),
            ));
        }
    };
    let file_name = pending.file_name.clone();
    let backup_path = match located_backup(backups_dir, &file_name) {
        Ok(path) => path,
        Err(error) => return Some(finish_with_error(data_dir, &file_name, None, error)),
    };
    if let Err(error) = validate_backup_file(&backup_path, pending.app_max_schema_version) {
        return Some(finish_with_error(data_dir, &file_name, None, error));
    }

    let now = time::OffsetDateTime::now_utc();
    let pre_restore = match snapshot_current_database(data_dir, backups_dir, now) {
        Ok(file_name) => file_name,
        Err(error) => return Some(finish_with_error(data_dir, &file_name, None, error)),
    };
    if let Err(error) = replace_database_file(data_dir, &backup_path) {
        return Some(finish_with_error(data_dir, &file_name, pre_restore, error));
    }

    let report = RestoreReport {
        ok: true,
        file_name: file_name.clone(),
        backup_created_at: created_at_for_named_file(&file_name, &backup_path),
        pre_restore_file_name: pre_restore,
        error: None,
    };
    Some(finish_restore(data_dir, report))
}

pub fn backup_status(
    data_dir: &Path,
    backups_dir: &Path,
    now: time::OffsetDateTime,
) -> BackupStatus {
    let backups = list_backups(backups_dir);
    let last_auto_at = backups
        .iter()
        .find(|item| item.kind == BackupKind::Auto)
        .map(|item| item.created_at.clone());
    let next_auto_at = match last_auto_at.as_deref().and_then(parse_rfc3339) {
        Some(last) => last
            .checked_add(time::Duration::hours(AUTO_BACKUP_INTERVAL_HOURS))
            .unwrap_or(now),
        None => now,
    };
    BackupStatus {
        last_auto_at,
        next_auto_at: format_rfc3339(next_auto_at),
        last_restore: read_restore_result(data_dir),
    }
}

pub(crate) fn parse_rfc3339(text: &str) -> Option<time::OffsetDateTime> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let (body, offset_seconds) = split_rfc3339_offset(text)?;
    let (date_part, time_part) = body.split_once('T')?;
    let time_part = time_part.split('.').next().unwrap_or(time_part);
    let mut date_fields = date_part.split('-');
    let year: i32 = date_fields.next()?.parse().ok()?;
    let month: u8 = date_fields.next()?.parse().ok()?;
    let day: u8 = date_fields.next()?.parse().ok()?;
    if date_fields.next().is_some() {
        return None;
    }
    let mut clock_fields = time_part.split(':');
    let hour: u8 = clock_fields.next()?.parse().ok()?;
    let minute: u8 = clock_fields.next()?.parse().ok()?;
    let second: u8 = clock_fields.next()?.parse().ok()?;
    if clock_fields.next().is_some() {
        return None;
    }
    let month = time::Month::try_from(month).ok()?;
    let date = time::Date::from_calendar_date(year, month, day).ok()?;
    let clock = time::Time::from_hms(hour, minute, second).ok()?;
    let offset = time::UtcOffset::from_whole_seconds(offset_seconds).ok()?;
    Some(time::PrimitiveDateTime::new(date, clock).assume_offset(offset))
}

fn publish_backup(db_path: &Path, partial: &Path, destination: &Path) -> Result<(), String> {
    if partial.exists() {
        remove_partial(partial);
    }
    vacuum_into(db_path, partial)?;
    if let Err(error) = integrity_is_ok(partial) {
        return Err(format!("备份完整性检查失败：{error}"));
    }
    #[cfg(test)]
    if FAIL_BEFORE_PUBLISH.with(|flag| flag.get()) {
        return Err("备份在写完临时文件后失败".to_owned());
    }
    if destination.exists() {
        return Err("备份文件名冲突，没有覆盖已有备份".to_owned());
    }
    fs::rename(partial, destination).map_err(|error| format!("备份改名失败：{error}"))?;
    Ok(())
}

fn vacuum_into(db_path: &Path, destination: &Path) -> Result<(), String> {
    let connection =
        Connection::open(db_path).map_err(|error| format!("数据库打开失败：{error}"))?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|error| format!("数据库忙等待设置失败：{error}"))?;
    let escaped = destination.to_string_lossy().replace('\'', "''");
    connection
        .execute_batch(&format!("VACUUM INTO '{escaped}'"))
        .map_err(|error| format!("备份失败：{error}"))?;
    Ok(())
}

fn reserve_backup_name(
    backups_dir: &Path,
    kind: BackupKind,
    now: time::OffsetDateTime,
) -> Result<String, String> {
    let base = backup_file_name(kind, now, None);
    if !name_taken(backups_dir, &base) {
        return Ok(base);
    }
    for index in 2..10_000 {
        let candidate = backup_file_name(kind, now, Some(index));
        if !name_taken(backups_dir, &candidate) {
            return Ok(candidate);
        }
    }
    Err("同一秒内备份太多，请稍后再试".to_owned())
}

fn name_taken(backups_dir: &Path, file_name: &str) -> bool {
    backups_dir.join(file_name).exists()
        || backups_dir.join(format!("{file_name}.partial")).exists()
}

fn backup_file_name(kind: BackupKind, now: time::OffsetDateTime, suffix: Option<u32>) -> String {
    let stamp = format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    );
    match suffix {
        None => format!("{}-{stamp}.sqlite3", kind_prefix(kind)),
        Some(index) => format!("{}-{stamp}-{index}.sqlite3", kind_prefix(kind)),
    }
}

fn kind_prefix(kind: BackupKind) -> &'static str {
    match kind {
        BackupKind::Auto => "auto",
        BackupKind::Manual => "manual",
        BackupKind::PreRestore => "pre-restore",
        BackupKind::PreMigrate => "pre-migrate",
    }
}

fn classify_backup_name(file_name: &str) -> Option<(BackupKind, Option<time::OffsetDateTime>)> {
    if !file_name.ends_with(".sqlite3") {
        return None;
    }
    let stem = file_name.trim_end_matches(".sqlite3");
    if let Some(index) = stem.rfind(".pre-migration-") {
        let stamp = &stem[index + ".pre-migration-".len()..];
        if !stamp.is_empty() && stamp.chars().all(|ch| ch.is_ascii_digit()) {
            return Some((BackupKind::PreMigrate, parse_epoch_digits(stamp)));
        }
    }
    let (kind, rest) = if let Some(rest) = stem.strip_prefix("auto-") {
        (BackupKind::Auto, rest)
    } else if let Some(rest) = stem.strip_prefix("pre-restore-") {
        (BackupKind::PreRestore, rest)
    } else if let Some(rest) = stem.strip_prefix("pre-migrate-") {
        (BackupKind::PreMigrate, rest)
    } else if let Some(rest) = stem.strip_prefix("manual-") {
        (BackupKind::Manual, rest)
    } else {
        return None;
    };
    if !rest.contains('-') && rest.chars().all(|ch| ch.is_ascii_digit()) {
        return Some((kind, parse_epoch_digits(rest)));
    }
    Some((kind, Some(parse_compact_stamp(rest)?)))
}

fn parse_compact_stamp(rest: &str) -> Option<time::OffsetDateTime> {
    let (date, tail) = rest.split_once('-')?;
    let hms = match tail.split_once('-') {
        Some((hms, suffix)) => {
            if suffix.is_empty() || !suffix.chars().all(|ch| ch.is_ascii_digit()) {
                return None;
            }
            hms
        }
        None => tail,
    };
    if date.len() != 8 || hms.len() != 6 {
        return None;
    }
    if !date.chars().all(|ch| ch.is_ascii_digit()) || !hms.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let year: i32 = date[0..4].parse().ok()?;
    let month: u8 = date[4..6].parse().ok()?;
    let day: u8 = date[6..8].parse().ok()?;
    let hour: u8 = hms[0..2].parse().ok()?;
    let minute: u8 = hms[2..4].parse().ok()?;
    let second: u8 = hms[4..6].parse().ok()?;
    let month = time::Month::try_from(month).ok()?;
    let date = time::Date::from_calendar_date(year, month, day).ok()?;
    let clock = time::Time::from_hms(hour, minute, second).ok()?;
    Some(time::PrimitiveDateTime::new(date, clock).assume_utc())
}

fn parse_epoch_digits(digits: &str) -> Option<time::OffsetDateTime> {
    if digits.len() < 10 || !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let value: i128 = digits.parse().ok()?;
    let seconds = if digits.len() >= 13 {
        value / 1000
    } else {
        value
    };
    let seconds = i64::try_from(seconds).ok()?;
    time::OffsetDateTime::from_unix_timestamp(seconds).ok()
}

fn reject_untrusted_file_name(file_name: &str) -> Result<(), String> {
    let path = Path::new(file_name);
    let invalid = file_name.is_empty()
        || file_name == "."
        || file_name.contains('/')
        || file_name.contains('\\')
        || file_name.contains('\0')
        || file_name.contains("..")
        || path.is_absolute()
        || path.components().count() != 1;
    if invalid {
        return Err("只能使用备份目录里的文件名，不能带路径".to_owned());
    }
    Ok(())
}

fn located_backup(backups_dir: &Path, file_name: &str) -> Result<PathBuf, String> {
    reject_untrusted_file_name(file_name)?;
    let path = backups_dir.join(file_name);
    if !path.starts_with(backups_dir) {
        return Err("只能使用备份目录里的文件名，不能带路径".to_owned());
    }
    Ok(path)
}

fn validate_backup_file(path: &Path, app_max_schema_version: u32) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "找不到这份备份".to_owned())?;
    if metadata.file_type().is_symlink() {
        return Err("不能恢复符号链接".to_owned());
    }
    if !metadata.is_file() {
        return Err("找不到这份备份".to_owned());
    }
    if let Err(error) = integrity_is_ok(path) {
        return Err(format!("这份备份已损坏，已拒绝恢复：{error}"));
    }
    let version = read_stats(path)
        .1
        .ok_or_else(|| "读不出这份备份的迁移版本，已拒绝恢复".to_owned())?;
    let number = version
        .trim()
        .parse::<u32>()
        .map_err(|_| "读不出这份备份的迁移版本，已拒绝恢复".to_owned())?;
    if number > app_max_schema_version {
        return Err(format!(
            "这份备份来自更新的版本（备份是 {version}，本应用只到 {app_max_schema_version:03}），已拒绝恢复"
        ));
    }
    Ok(())
}

fn integrity_is_ok(path: &Path) -> Result<(), String> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| format!("打开失败：{error}"))?;
    let _ = connection.execute_batch("PRAGMA query_only = ON;");
    let result: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| format!("校验失败：{error}"))?;
    if result != "ok" {
        return Err(format!("完整性不是 ok：{result}"));
    }
    Ok(())
}

fn read_stats(path: &Path) -> (Option<i64>, Option<String>) {
    let Ok(connection) = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) else {
        return (None, None);
    };
    let _ = connection.execute_batch("PRAGMA query_only = ON;");
    let repository_count = connection
        .query_row(
            "SELECT COUNT(*) FROM repositories WHERE sync_status = 'active'",
            [],
            |row| row.get(0),
        )
        .ok();
    let schema_version = connection
        .query_row(
            "SELECT CAST(MAX(version) AS TEXT) FROM schema_migrations",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
        .filter(|version| !version.is_empty());
    (repository_count, schema_version)
}

fn snapshot_current_database(
    data_dir: &Path,
    backups_dir: &Path,
    now: time::OffsetDateTime,
) -> Result<Option<String>, String> {
    let db_path = data_dir.join(DATABASE_FILE_NAME);
    if !db_path.is_file() {
        return Ok(None);
    }
    fs::create_dir_all(backups_dir).map_err(|error| format!("备份目录创建失败：{error}"))?;
    let scratch_db = stage_consistent_copy(&db_path)?;
    let scratch_dir = scratch_db.parent().map(Path::to_path_buf);
    let result = create_backup_at(&scratch_db, backups_dir, BackupKind::PreRestore, now);
    if let Some(dir) = scratch_dir {
        let _ = fs::remove_dir_all(dir);
    }
    result.map(|info| Some(info.file_name))
}

/// 把活动库连同 `-wal` / `-shm` 拷到临时目录再导出，避免打开活动库本身。
fn stage_consistent_copy(db_path: &Path) -> Result<PathBuf, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let scratch =
        std::env::temp_dir().join(format!("fsl-pre-restore-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&scratch).map_err(|error| format!("恢复前备份临时目录创建失败：{error}"))?;
    let scratch_db = scratch.join(DATABASE_FILE_NAME);
    let copied = (|| -> Result<(), String> {
        fs::copy(db_path, &scratch_db).map_err(|error| format!("复制当前资料库失败：{error}"))?;
        for suffix in ["-wal", "-shm"] {
            let Some(source) = sidecar(db_path, suffix) else {
                continue;
            };
            if !source.is_file() {
                continue;
            }
            let Some(destination) = sidecar(&scratch_db, suffix) else {
                continue;
            };
            fs::copy(&source, &destination)
                .map_err(|error| format!("复制资料库辅助文件失败：{error}"))?;
        }
        Ok(())
    })();
    if let Err(error) = copied {
        let _ = fs::remove_dir_all(&scratch);
        return Err(error);
    }
    Ok(scratch_db)
}

fn replace_database_file(data_dir: &Path, backup_path: &Path) -> Result<(), String> {
    let db_path = data_dir.join(DATABASE_FILE_NAME);
    let partial = data_dir.join(format!("{DATABASE_FILE_NAME}.restore-partial"));
    if partial.exists() {
        remove_partial(&partial);
    }
    fs::copy(backup_path, &partial).map_err(|error| format!("复制备份失败：{error}"))?;
    if let Err(error) = integrity_is_ok(&partial) {
        remove_partial(&partial);
        return Err(format!("这份备份已损坏，已拒绝恢复：{error}"));
    }

    let wal = sidecar(&db_path, "-wal");
    let shm = sidecar(&db_path, "-shm");
    let wal_aside = data_dir.join(format!("{DATABASE_FILE_NAME}.wal-aside"));
    let shm_aside = data_dir.join(format!("{DATABASE_FILE_NAME}.shm-aside"));
    let moved_wal = match move_aside(wal.as_deref(), &wal_aside) {
        Ok(moved) => moved,
        Err(error) => {
            remove_partial(&partial);
            return Err(error);
        }
    };
    let moved_shm = match move_aside(shm.as_deref(), &shm_aside) {
        Ok(moved) => moved,
        Err(error) => {
            restore_aside(moved_wal, &wal_aside, wal.as_deref());
            remove_partial(&partial);
            return Err(error);
        }
    };
    if let Err(error) = fs::rename(&partial, &db_path) {
        restore_aside(moved_wal, &wal_aside, wal.as_deref());
        restore_aside(moved_shm, &shm_aside, shm.as_deref());
        remove_partial(&partial);
        return Err(format!("替换资料库失败：{error}"));
    }
    if wal_aside.exists() {
        let _ = fs::remove_file(&wal_aside);
    }
    if shm_aside.exists() {
        let _ = fs::remove_file(&shm_aside);
    }
    Ok(())
}

fn move_aside(source: Option<&Path>, aside: &Path) -> Result<bool, String> {
    let Some(source) = source else {
        return Ok(false);
    };
    if !source.exists() {
        return Ok(false);
    }
    if aside.exists() {
        fs::remove_file(aside).map_err(|error| format!("清理临时文件失败：{error}"))?;
    }
    fs::rename(source, aside).map_err(|error| format!("移开资料库辅助文件失败：{error}"))?;
    Ok(true)
}

fn restore_aside(moved: bool, aside: &Path, original: Option<&Path>) {
    if !moved {
        return;
    }
    let Some(original) = original else {
        return;
    };
    let _ = fs::rename(aside, original);
}

fn finish_with_error(
    data_dir: &Path,
    file_name: &str,
    pre_restore_file_name: Option<String>,
    error: String,
) -> RestoreReport {
    finish_restore(
        data_dir,
        RestoreReport {
            ok: false,
            file_name: file_name.to_owned(),
            backup_created_at: None,
            pre_restore_file_name,
            error: Some(error),
        },
    )
}

fn finish_restore(data_dir: &Path, report: RestoreReport) -> RestoreReport {
    if let Err(error) = write_json(&data_dir.join(RESTORE_RESULT_FILE), &report) {
        eprintln!("写入恢复结果失败：{error}");
    }
    let _ = fs::remove_file(data_dir.join(PENDING_RESTORE_FILE));
    report
}

fn read_restore_result(data_dir: &Path) -> Option<RestoreReport> {
    let path = data_dir.join(RESTORE_RESULT_FILE);
    if !path.is_file() {
        return None;
    }
    read_json(&path).ok()
}

fn created_at_for_named_file(file_name: &str, path: &Path) -> Option<String> {
    let (_kind, from_name) = classify_backup_name(file_name)?;
    if let Some(created) = from_name {
        return Some(format_rfc3339(created));
    }
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(system_time_to_rfc3339)
}

fn backup_info(
    path: &Path,
    kind: BackupKind,
    file_name: &str,
    created: time::OffsetDateTime,
) -> Result<BackupInfo, String> {
    let bytes = fs::metadata(path)
        .map_err(|error| format!("备份文件读取失败：{error}"))?
        .len();
    let (repository_count, schema_version) = read_stats(path);
    Ok(BackupInfo {
        file_name: file_name.to_owned(),
        kind,
        created_at: format_rfc3339(created),
        bytes,
        repository_count,
        schema_version,
    })
}

fn remove_partial(partial: &Path) {
    let _ = fs::remove_file(partial);
    for suffix in ["-wal", "-shm", "-journal"] {
        if let Some(path) = sidecar(partial, suffix) {
            let _ = fs::remove_file(path);
        }
    }
}

fn sidecar(path: &Path, suffix: &str) -> Option<PathBuf> {
    let mut name = path.file_name()?.to_os_string();
    name.push(suffix);
    Some(path.with_file_name(name))
}

fn format_rfc3339(value: time::OffsetDateTime) -> String {
    let value = value.to_offset(time::UtcOffset::UTC);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        value.year(),
        u8::from(value.month()),
        value.day(),
        value.hour(),
        value.minute(),
        value.second()
    )
}

fn system_time_to_rfc3339(value: SystemTime) -> Option<String> {
    let duration = value.duration_since(UNIX_EPOCH).ok()?;
    let seconds = i64::try_from(duration.as_secs()).ok()?;
    let created = time::OffsetDateTime::from_unix_timestamp(seconds).ok()?;
    Some(format_rfc3339(created))
}

fn split_rfc3339_offset(text: &str) -> Option<(&str, i32)> {
    if let Some(body) = text.strip_suffix('Z') {
        return Some((body, 0));
    }
    let split_at = text.rfind(['+', '-'])?;
    if split_at < 11 {
        return None;
    }
    let body = &text[..split_at];
    let offset = &text[split_at..];
    let sign: i32 = if offset.starts_with('+') { 1 } else { -1 };
    let numbers = offset[1..].split(':').collect::<Vec<_>>();
    if numbers.len() != 2 {
        return None;
    }
    let hours: i32 = numbers[0].parse().ok()?;
    let minutes: i32 = numbers[1].parse().ok()?;
    Some((body, sign * (hours * 3600 + minutes * 60)))
}

fn recent_month_keys(now: time::OffsetDateTime, count: usize) -> Vec<(i32, u8)> {
    let mut year = now.year();
    let mut month = u8::from(now.month());
    let mut keys = Vec::with_capacity(count);
    for _ in 0..count {
        keys.push((year, month));
        if month == 1 {
            year -= 1;
            month = 12;
        } else {
            month -= 1;
        }
    }
    keys
}

fn backup_month(entry: &BackupInfo) -> Option<(i32, u8)> {
    let created = parse_rfc3339(&entry.created_at)?;
    Some((created.year(), u8::from(created.month())))
}

fn backup_time_asc(left: &BackupInfo, right: &BackupInfo) -> std::cmp::Ordering {
    parse_rfc3339(&left.created_at)
        .cmp(&parse_rfc3339(&right.created_at))
        .then_with(|| left.file_name.cmp(&right.file_name))
}

fn backup_time_desc(left: &BackupInfo, right: &BackupInfo) -> std::cmp::Ordering {
    backup_time_asc(right, left)
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("write");
    let partial = path.with_file_name(format!("{file_name}.partial"));
    let text = serde_json::to_string_pretty(value).map_err(|error| format!("编码失败：{error}"))?;
    if let Err(error) = fs::write(&partial, text) {
        let _ = fs::remove_file(&partial);
        return Err(format!("写入失败：{error}"));
    }
    if let Err(error) = fs::rename(&partial, path) {
        let _ = fs::remove_file(&partial);
        return Err(format!("写入失败：{error}"));
    }
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("读取失败：{error}"))?;
    serde_json::from_str(&text).map_err(|error| format!("解析失败：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("系统时间应可用")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("fsl-backup-{label}-{nanos}"));
            fs::create_dir_all(&path).expect("应能创建临时目录");
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct FailBeforePublish;

    impl FailBeforePublish {
        fn arm() -> Self {
            FAIL_BEFORE_PUBLISH.with(|flag| flag.set(true));
            Self
        }
    }

    impl Drop for FailBeforePublish {
        fn drop(&mut self) {
            FAIL_BEFORE_PUBLISH.with(|flag| flag.set(false));
        }
    }

    fn write_sample_db(path: &Path, version: &str, marker: &str, active_count: i64) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("应能创建库目录");
        }
        let connection = Connection::open(path).expect("应能创建测试库");
        connection
            .execute_batch(&format!(
                "
                CREATE TABLE schema_migrations(version TEXT, name TEXT);
                INSERT INTO schema_migrations(version, name) VALUES ('{version}', 'test');
                CREATE TABLE repositories(id TEXT, sync_status TEXT);
                CREATE TABLE marker(v TEXT);
                INSERT INTO marker(v) VALUES ('{marker}');
                "
            ))
            .expect("应能写入测试表");
        for index in 0..active_count {
            connection
                .execute(
                    "INSERT INTO repositories(id, sync_status) VALUES (?1, 'active')",
                    [format!("active-{index}")],
                )
                .expect("应能写入有效仓库");
        }
        connection
            .execute(
                "INSERT INTO repositories(id, sync_status) VALUES ('gone', 'removed')",
                [],
            )
            .expect("应能写入已移除仓库");
    }

    fn write_wal_db(path: &Path, marker: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("应能创建库目录");
        }
        let connection = Connection::open(path).expect("应能创建 WAL 测试库");
        connection
            .set_db_config(
                rusqlite::config::DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE,
                true,
            )
            .expect("应能关闭关闭时检查点");
        connection
            .execute_batch(&format!(
                "
                PRAGMA journal_mode = WAL;
                PRAGMA wal_autocheckpoint = 0;
                CREATE TABLE schema_migrations(version TEXT, name TEXT);
                INSERT INTO schema_migrations(version, name) VALUES ('029', 'test');
                CREATE TABLE marker(v TEXT);
                INSERT INTO marker(v) VALUES ('{marker}');
                "
            ))
            .expect("应能写入 WAL 测试库");
    }

    fn marker_of(path: &Path) -> String {
        let connection = Connection::open(path).expect("应能打开库");
        connection
            .query_row("SELECT v FROM marker", [], |row| row.get(0))
            .expect("应能读出标记")
    }

    fn assert_no_partial(dir: &Path) {
        let entries = fs::read_dir(dir).expect("应能读取目录");
        for entry in entries {
            let name = entry.expect("应能读取目录项").file_name();
            let name = name.to_string_lossy();
            assert!(!name.contains("partial"), "留下了半个文件：{name}");
        }
    }

    fn entry(file_name: &str, kind: BackupKind, created_at: &str) -> BackupInfo {
        BackupInfo {
            file_name: file_name.to_owned(),
            kind,
            created_at: created_at.to_owned(),
            bytes: 8,
            repository_count: Some(1),
            schema_version: Some("029".to_owned()),
        }
    }

    fn auto_at(day: &str, hour: u8) -> BackupInfo {
        let file_name = format!("auto-202610{day}-{hour:02}0000.sqlite3");
        entry(
            &file_name,
            BackupKind::Auto,
            &format!("2026-10-{day}T{hour:02}:00:00Z"),
        )
    }

    fn write_pending(data_dir: &Path, file_name: &str, app_max: u32) {
        let pending = PendingRestore {
            file_name: file_name.to_owned(),
            requested_at: "2026-10-03T00:00:00Z".to_owned(),
            app_max_schema_version: app_max,
        };
        write_json(&data_dir.join(PENDING_RESTORE_FILE), &pending).expect("应能写入待恢复标记");
    }

    #[test]
    fn database_file_name_matches_storage() {
        assert_eq!(
            DATABASE_FILE_NAME,
            crate::storage::SQLITE_DATABASE_FILE_NAME
        );
    }

    #[test]
    fn create_backup_passes_integrity_check_and_leaves_no_partial() {
        let dir = TempDir::new("create-ok");
        let db_path = dir.0.join("source.sqlite3");
        let backups = dir.0.join("backups");
        write_sample_db(&db_path, "018", "kept", 2);

        let info = create_backup(&db_path, &backups, BackupKind::Manual).expect("应能创建备份");

        assert_eq!(info.kind, BackupKind::Manual);
        assert_eq!(info.repository_count, Some(2));
        assert_eq!(info.schema_version.as_deref(), Some("018"));
        assert!(info.bytes > 0);
        assert!(info.file_name.starts_with("manual-"));
        assert!(info.file_name.ends_with(".sqlite3"));
        assert!(info.created_at.ends_with('Z'));
        let backup_path = backups.join(&info.file_name);
        assert!(backup_path.is_file());
        integrity_is_ok(&backup_path).expect("新备份应通过完整性检查");
        assert_eq!(marker_of(&backup_path), "kept");
        assert_no_partial(&backups);
    }

    #[test]
    fn create_backup_failure_after_partial_leaves_no_partial() {
        let dir = TempDir::new("create-fail");
        let db_path = dir.0.join("source.sqlite3");
        let backups = dir.0.join("backups");
        write_sample_db(&db_path, "029", "kept", 1);
        let _guard = FailBeforePublish::arm();

        let error =
            create_backup(&db_path, &backups, BackupKind::Auto).expect_err("应在改名前失败");

        assert!(error.contains("写完临时文件后失败"), "{error}");
        assert_no_partial(&backups);
        let published = fs::read_dir(&backups)
            .expect("应能读取备份目录")
            .filter_map(|entry| entry.ok())
            .count();
        assert_eq!(published, 0);
    }

    #[test]
    fn create_backup_invalid_source_leaves_no_partial() {
        let dir = TempDir::new("create-bad-source");
        let db_path = dir.0.join("not-a-database.sqlite3");
        let backups = dir.0.join("backups");
        fs::create_dir_all(&backups).expect("应能创建备份目录");
        fs::write(&db_path, b"this is not sqlite").expect("应能写入坏文件");

        let error = create_backup(&db_path, &backups, BackupKind::Manual).expect_err("坏库应失败");

        assert!(!error.is_empty());
        assert_no_partial(&backups);
        assert_eq!(fs::read_dir(&backups).expect("应能读取备份目录").count(), 0);
    }

    #[test]
    fn create_backup_same_second_does_not_overwrite() {
        let dir = TempDir::new("create-same-second");
        let db_path = dir.0.join("source.sqlite3");
        let backups = dir.0.join("backups");
        write_sample_db(&db_path, "029", "first", 1);
        let now = time::OffsetDateTime::from_unix_timestamp(1_759_500_000).expect("时间戳应有效");

        let first =
            create_backup_at(&db_path, &backups, BackupKind::Manual, now).expect("第一次应成功");
        let first_bytes = fs::read(backups.join(&first.file_name)).expect("应能读取第一份备份");
        let second =
            create_backup_at(&db_path, &backups, BackupKind::Manual, now).expect("第二次应成功");

        assert_eq!(
            first.file_name,
            backup_file_name(
                BackupKind::Manual,
                now.replace_nanosecond(0).unwrap_or(now),
                None
            )
        );
        assert_eq!(
            second.file_name,
            backup_file_name(
                BackupKind::Manual,
                now.replace_nanosecond(0).unwrap_or(now),
                Some(2)
            )
        );
        assert_ne!(first.file_name, second.file_name);
        assert_eq!(
            fs::read(backups.join(&first.file_name)).expect("第一份备份应还在"),
            first_bytes
        );
        assert!(backups.join(&second.file_name).is_file());
        assert_no_partial(&backups);
    }

    #[test]
    fn retention_plan_table() {
        let now = parse_rfc3339("2026-10-20T12:00:00Z").expect("基准时间应可解析");
        let october_fourteen: Vec<BackupInfo> = (0..14).map(|hour| auto_at("20", hour)).collect();
        let mut october_fifteen = october_fourteen.clone();
        october_fifteen.push(auto_at("20", 14));

        let mut across_months = Vec::new();
        for day in [
            "20", "19", "18", "17", "16", "15", "14", "13", "12", "11", "10", "09", "08", "07",
        ] {
            across_months.push(entry(
                &format!("auto-202610{day}-120000.sqlite3"),
                BackupKind::Auto,
                &format!("2026-10-{day}T12:00:00Z"),
            ));
        }
        across_months.push(entry(
            "auto-20260928-120000.sqlite3",
            BackupKind::Auto,
            "2026-09-28T12:00:00Z",
        ));
        across_months.push(entry(
            "auto-20260915-120000.sqlite3",
            BackupKind::Auto,
            "2026-09-15T12:00:00Z",
        ));
        across_months.push(entry(
            "auto-20260902-120000.sqlite3",
            BackupKind::Auto,
            "2026-09-02T12:00:00Z",
        ));
        across_months.push(entry(
            "auto-20260830-120000.sqlite3",
            BackupKind::Auto,
            "2026-08-30T12:00:00Z",
        ));
        across_months.push(entry(
            "auto-20260801-120000.sqlite3",
            BackupKind::Auto,
            "2026-08-01T12:00:00Z",
        ));
        across_months.push(entry(
            "auto-20260731-120000.sqlite3",
            BackupKind::Auto,
            "2026-07-31T12:00:00Z",
        ));
        across_months.push(entry(
            "manual-20200101-000000.sqlite3",
            BackupKind::Manual,
            "2020-01-01T00:00:00Z",
        ));
        across_months.push(entry(
            "pre-restore-20200101-000000.sqlite3",
            BackupKind::PreRestore,
            "2020-01-01T00:00:00Z",
        ));
        across_months.push(entry(
            "fox-stars-lab.sqlite3.pre-migration-1577836800000.sqlite3",
            BackupKind::PreMigrate,
            "2020-01-01T00:00:00Z",
        ));

        let mut protected_with_old_autos = vec![
            entry(
                "manual-20190101-000000.sqlite3",
                BackupKind::Manual,
                "2019-01-01T00:00:00Z",
            ),
            entry(
                "pre-restore-20190101-000000.sqlite3",
                BackupKind::PreRestore,
                "2019-01-01T00:00:00Z",
            ),
            entry(
                "pre-migrate-20190101-000000.sqlite3",
                BackupKind::PreMigrate,
                "2019-01-01T00:00:00Z",
            ),
        ];
        for day in 1..=16 {
            protected_with_old_autos.push(entry(
                &format!("auto-202001{day:02}-000000.sqlite3"),
                BackupKind::Auto,
                &format!("2020-01-{day:02}T00:00:00Z"),
            ));
        }

        let cases = [
            ("恰好 14 份", october_fourteen, Vec::<String>::new()),
            (
                "超过 14 份",
                october_fifteen,
                vec!["auto-20261020-000000.sqlite3".to_owned()],
            ),
            (
                "跨月每月留最新一份",
                across_months,
                vec![
                    "auto-20260731-120000.sqlite3".to_owned(),
                    "auto-20260801-120000.sqlite3".to_owned(),
                    "auto-20260902-120000.sqlite3".to_owned(),
                    "auto-20260915-120000.sqlite3".to_owned(),
                ],
            ),
            (
                "手动、恢复前、迁移前永不删除",
                protected_with_old_autos,
                vec![
                    "auto-20200101-000000.sqlite3".to_owned(),
                    "auto-20200102-000000.sqlite3".to_owned(),
                ],
            ),
            ("空列表", Vec::new(), Vec::new()),
        ];

        for (name, entries, expected) in cases {
            let mut actual = plan_retention(&entries, now);
            let mut expected = expected.clone();
            actual.sort();
            expected.sort();
            assert_eq!(actual, expected, "{name}");
            assert!(
                actual
                    .iter()
                    .all(|file_name| file_name.starts_with("auto-")),
                "{name} 删到了非自动备份：{actual:?}"
            );
        }
    }

    #[test]
    fn auto_backup_due_boundaries() {
        let now = time::OffsetDateTime::from_unix_timestamp(1_800_000_000).expect("时间戳应有效");
        let under = now
            .checked_sub(time::Duration::hours(19) + time::Duration::minutes(59))
            .expect("19 小时 59 分应能相减");
        let exact = now
            .checked_sub(time::Duration::hours(20))
            .expect("20 小时应能相减");

        assert!(!auto_backup_due(Some(under), now));
        assert!(auto_backup_due(Some(exact), now));
        assert!(auto_backup_due(None, now));
    }

    #[test]
    fn list_backups_sorts_kinds_and_skips_unknown() {
        let dir = TempDir::new("list");
        let backups = dir.0.join("backups");
        fs::create_dir_all(&backups).expect("应能创建备份目录");
        write_sample_db(
            &backups.join("auto-20261003-140000.sqlite3"),
            "029",
            "auto",
            2,
        );
        write_sample_db(
            &backups.join("auto-20261003-140000-2.sqlite3"),
            "029",
            "auto-2",
            1,
        );
        write_sample_db(
            &backups.join("manual-20261003-150000.sqlite3"),
            "029",
            "manual",
            1,
        );
        write_sample_db(
            &backups.join("pre-restore-20261002-090000.sqlite3"),
            "028",
            "pre",
            1,
        );
        write_sample_db(
            &backups.join("fox-stars-lab.sqlite3.pre-migration-1720000000000.sqlite3"),
            "018",
            "migration",
            1,
        );
        fs::write(
            backups.join("manual-1710000000000.sqlite3"),
            b"not a database",
        )
        .expect("应能写入旧式坏备份");
        fs::write(
            backups.join("auto-20261003-140000.sqlite3.partial"),
            b"half",
        )
        .expect("应能写入半成品");
        fs::write(backups.join("notes.txt"), b"ignore").expect("应能写入未知文件");
        fs::write(backups.join("backup.sqlite3"), b"unknown").expect("应能写入未知库文件");

        let listed = list_backups(&backups);
        let names: Vec<&str> = listed.iter().map(|item| item.file_name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "manual-20261003-150000.sqlite3",
                "auto-20261003-140000.sqlite3",
                "auto-20261003-140000-2.sqlite3",
                "pre-restore-20261002-090000.sqlite3",
                "fox-stars-lab.sqlite3.pre-migration-1720000000000.sqlite3",
                "manual-1710000000000.sqlite3",
            ]
        );
        assert_eq!(
            listed.iter().map(|item| item.kind).collect::<Vec<_>>(),
            vec![
                BackupKind::Manual,
                BackupKind::Auto,
                BackupKind::Auto,
                BackupKind::PreRestore,
                BackupKind::PreMigrate,
                BackupKind::Manual,
            ]
        );
        assert_eq!(listed[1].repository_count, Some(2));
        assert_eq!(listed[1].schema_version.as_deref(), Some("029"));
        assert_eq!(listed[1].created_at, "2026-10-03T14:00:00Z");
        assert_eq!(listed[5].repository_count, None);
        assert_eq!(listed[5].schema_version, None);
        let migration_at = parse_epoch_digits("1720000000000").expect("迁移时间戳应可解析");
        assert_eq!(listed[4].created_at, format_rfc3339(migration_at));
    }

    #[test]
    fn request_restore_rejects_path_traversal() {
        let dir = TempDir::new("reject-path");
        let data_dir = dir.0.join("data");
        let backups = data_dir.join("backups");
        fs::create_dir_all(&backups).expect("应能创建目录");
        let outside = dir.0.join("outside.txt");
        fs::write(&outside, b"sentinel").expect("应能写入外部文件");
        let names = ["../x", "a/b", "/tmp/x.sqlite3", r"..\x", "foo..bar.sqlite3"];

        for file_name in names {
            let error = request_restore(&data_dir, &backups, file_name, 29)
                .expect_err("带路径的名字应被拒绝");
            assert!(error.contains("不能带路径"), "{file_name}: {error}");
        }
        assert!(!data_dir.join(PENDING_RESTORE_FILE).exists());
        assert_eq!(fs::read(&outside).expect("外部文件应还在"), b"sentinel");
    }

    #[test]
    fn request_restore_rejects_corrupt_backup() {
        let dir = TempDir::new("reject-corrupt");
        let data_dir = dir.0.join("data");
        let backups = data_dir.join("backups");
        fs::create_dir_all(&backups).expect("应能创建目录");
        fs::write(backups.join("manual-20261003-120000.sqlite3"), b"broken")
            .expect("应能写入损坏备份");

        let error = request_restore(&data_dir, &backups, "manual-20261003-120000.sqlite3", 29)
            .expect_err("损坏备份应被拒绝");

        assert!(error.contains("损坏"), "{error}");
        assert!(!data_dir.join(PENDING_RESTORE_FILE).exists());
    }

    #[test]
    fn request_restore_rejects_newer_schema() {
        let dir = TempDir::new("reject-version");
        let data_dir = dir.0.join("data");
        let backups = data_dir.join("backups");
        let file_name = "manual-20261003-120000.sqlite3";
        write_sample_db(&backups.join(file_name), "030", "newer", 1);

        let error =
            request_restore(&data_dir, &backups, file_name, 29).expect_err("更高版本应被拒绝");

        assert!(error.contains("这份备份来自更新的版本"), "{error}");
        assert!(!data_dir.join(PENDING_RESTORE_FILE).exists());
    }

    #[test]
    fn request_restore_accepts_valid_backup_without_touching_database() {
        let dir = TempDir::new("accept-restore");
        let data_dir = dir.0.join("data");
        let backups = data_dir.join("backups");
        let db_path = data_dir.join(DATABASE_FILE_NAME);
        fs::create_dir_all(&data_dir).expect("应能创建数据目录");
        fs::write(&db_path, b"live-bytes").expect("应能写入活动库");
        let before = fs::read(&db_path).expect("应能读取活动库");
        let file_name = "manual-20261003-120000.sqlite3";
        write_sample_db(&backups.join(file_name), "029", "backup", 1);

        request_restore(&data_dir, &backups, file_name, 29).expect("正常备份应被接受");

        assert_eq!(fs::read(&db_path).expect("活动库应还在"), before);
        let pending: PendingRestore =
            read_json(&data_dir.join(PENDING_RESTORE_FILE)).expect("应写下待恢复标记");
        assert_eq!(pending.file_name, file_name);
        assert_eq!(pending.app_max_schema_version, 29);
        assert!(!data_dir.join(RESTORE_RESULT_FILE).exists());
    }

    #[test]
    fn apply_pending_restore_replaces_database_and_saves_pre_restore() {
        let dir = TempDir::new("apply-ok");
        let data_dir = dir.0.join("data");
        let backups = data_dir.join("backups");
        fs::create_dir_all(&backups).expect("应能创建备份目录");
        let db_path = data_dir.join(DATABASE_FILE_NAME);
        write_wal_db(&db_path, "before");
        let wal = sidecar(&db_path, "-wal").expect("应有 wal 路径");
        let shm = sidecar(&db_path, "-shm").expect("应有 shm 路径");
        assert!(wal.is_file(), "关闭连接后 wal 应仍在，恢复才需要把它删掉");
        assert!(shm.is_file(), "关闭连接后 shm 应仍在");
        let file_name = "manual-20261003-120000.sqlite3";
        write_sample_db(&backups.join(file_name), "029", "after", 0);
        write_pending(&data_dir, file_name, 29);

        let report = apply_pending_restore(&data_dir, &backups).expect("有标记时应返回报告");

        assert!(report.ok, "{report:?}");
        assert_eq!(report.error, None);
        assert_eq!(report.file_name, file_name);
        assert_eq!(
            report.backup_created_at.as_deref(),
            Some("2026-10-03T12:00:00Z")
        );
        let pre_name = report.pre_restore_file_name.expect("应生成恢复前备份");
        assert!(pre_name.starts_with("pre-restore-"));
        assert_eq!(marker_of(&db_path), "after");
        assert_eq!(marker_of(&backups.join(&pre_name)), "before");
        assert!(!wal.exists());
        assert!(!shm.exists());
        assert!(!data_dir.join(PENDING_RESTORE_FILE).exists());
        let result =
            fs::read_to_string(data_dir.join(RESTORE_RESULT_FILE)).expect("应写下恢复结果");
        assert!(result.contains(file_name));
        assert_no_partial(&backups);
    }

    #[test]
    fn apply_pending_restore_corrupt_backup_keeps_database_bytes() {
        let dir = TempDir::new("apply-corrupt");
        let data_dir = dir.0.join("data");
        let backups = data_dir.join("backups");
        fs::create_dir_all(&backups).expect("应能创建备份目录");
        let db_path = data_dir.join(DATABASE_FILE_NAME);
        let original = b"live-database-bytes-v1".to_vec();
        fs::write(&db_path, &original).expect("应能写入活动库");
        let file_name = "manual-20261003-120000.sqlite3";
        fs::write(backups.join(file_name), b"broken-backup").expect("应能写入损坏备份");
        write_pending(&data_dir, file_name, 29);

        let report = apply_pending_restore(&data_dir, &backups).expect("损坏备份也应返回报告");

        assert!(!report.ok);
        let error = report.error.expect("应有失败原因");
        assert!(error.contains("损坏"), "{error}");
        assert_eq!(fs::read(&db_path).expect("活动库应还在"), original);
        assert!(!data_dir.join(PENDING_RESTORE_FILE).exists());
        let result =
            fs::read_to_string(data_dir.join(RESTORE_RESULT_FILE)).expect("应写下失败原因");
        assert!(result.contains("损坏"), "{result}");
        let pre_restore_count = fs::read_dir(&backups)
            .expect("应能读取备份目录")
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("pre-restore-")
            })
            .count();
        assert_eq!(pre_restore_count, 0);
    }

    #[test]
    fn apply_pending_restore_without_marker_does_nothing() {
        let dir = TempDir::new("apply-none");
        let data_dir = dir.0.join("data");
        fs::create_dir_all(&data_dir).expect("应能创建数据目录");
        let db_path = data_dir.join(DATABASE_FILE_NAME);
        let original = b"untouched-database".to_vec();
        fs::write(&db_path, &original).expect("应能写入活动库");
        let backups = data_dir.join("backups");
        let before = fs::read_dir(&data_dir)
            .expect("应能读取数据目录")
            .map(|entry| entry.expect("应能读取目录项").file_name())
            .collect::<Vec<_>>();

        let report = apply_pending_restore(&data_dir, &backups);

        assert!(report.is_none());
        assert_eq!(fs::read(&db_path).expect("活动库应还在"), original);
        assert!(!backups.exists());
        assert!(!data_dir.join(RESTORE_RESULT_FILE).exists());
        assert!(!data_dir.join(PENDING_RESTORE_FILE).exists());
        let after = fs::read_dir(&data_dir)
            .expect("应能读取数据目录")
            .map(|entry| entry.expect("应能读取目录项").file_name())
            .collect::<Vec<_>>();
        assert_eq!(before, after);
    }
}
