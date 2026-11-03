use git2::{Delta, Diff, DiffOptions, Repository, Tree};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Typechange,
    Untracked,
}

impl ChangeKind {
    pub fn label(&self) -> &'static str {
        match self {
            ChangeKind::Added => "A",
            ChangeKind::Modified => "M",
            ChangeKind::Deleted => "D",
            ChangeKind::Renamed => "R",
            ChangeKind::Copied => "C",
            ChangeKind::Typechange => "T",
            ChangeKind::Untracked => "?",
        }
    }
}

/// 宣言順がそのまま一覧の並び順になる（staged が先）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    Staged,
    Unstaged,
}

impl Stage {
    pub fn icon(&self) -> &'static str {
        match self {
            Stage::Staged => "●",
            Stage::Unstaged => "○",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: String,
    pub kind: ChangeKind,
    pub stage: Stage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Addition,
    Deletion,
    HunkHeader,
    FileHeader,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
}


/// 作業ツリーの差分を読み出す口。
///
/// 失敗の理由を呼び出し側に返すのがこの境界の役目で、`Result` を捨てると
/// 「変更が無い」と見分けがつかなくなる。テストでは実リポジトリの代わりに
/// 任意の結果を返す実装を差し込む。
pub trait DiffSource {
    /// staged / unstaged / untracked の変更ファイル一覧。
    fn changed_files(&self) -> Result<Vec<FileEntry>, git2::Error>;

    /// 1 ファイルぶんの差分を行単位で取得する。
    fn file_diff(&self, entry: &FileEntry) -> Result<Vec<DiffLine>, git2::Error>;
}

pub struct GitRepo {
    repo: Repository,
}

const MAX_DIFF_LINES: usize = 10_000;

impl GitRepo {
    pub fn open(path: Option<&str>) -> Result<Self, git2::Error> {
        let repo = match path {
            Some(p) => Repository::discover(p)?,
            None => Repository::discover(".")?,
        };
        Ok(Self { repo })
    }

    /// HEAD のツリー。コミットが 1 つも無いリポジトリでは `None` になり、
    /// 空のツリーとの差分として扱われる。
    fn head_tree(&self) -> Option<Tree<'_>> {
        self.repo.head().and_then(|r| r.peel_to_tree()).ok()
    }

    fn diff_for_entry(&self, entry: &FileEntry) -> Result<Diff<'_>, git2::Error> {
        let mut opts = DiffOptions::new();
        opts.pathspec(&entry.path);
        opts.include_untracked(true);
        opts.recurse_untracked_dirs(true);

        match entry.stage {
            Stage::Staged => {
                self.repo
                    .diff_tree_to_index(self.head_tree().as_ref(), None, Some(&mut opts))
            }
            Stage::Unstaged => self.repo.diff_index_to_workdir(None, Some(&mut opts)),
        }
    }

    fn untracked_file_lines(&self, entry: &FileEntry) -> Vec<DiffLine> {
        let workdir = match self.repo.workdir() {
            Some(d) => d,
            None => return vec![],
        };

        let full_path = workdir.join(&entry.path);
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => {
                return vec![DiffLine {
                    kind: DiffLineKind::Context,
                    content: "Binary file or unreadable".to_string(),
                }];
            }
        };

        let mut lines = vec![DiffLine {
            kind: DiffLineKind::FileHeader,
            content: format!("new file: {}\n", entry.path),
        }];

        for line in content.lines().take(MAX_DIFF_LINES) {
            lines.push(DiffLine {
                kind: DiffLineKind::Addition,
                content: format!("{line}\n"),
            });
        }

        lines
    }
}

impl DiffSource for GitRepo {
    fn changed_files(&self) -> Result<Vec<FileEntry>, git2::Error> {
        let staged = self.repo.diff_tree_to_index(
            self.head_tree().as_ref(),
            None,
            Some(&mut DiffOptions::new()),
        )?;
        let unstaged = self
            .repo
            .diff_index_to_workdir(None, Some(DiffOptions::new().include_untracked(true)))?;

        let mut entries = entries_from_diff(&staged, Stage::Staged);
        entries.extend(entries_from_diff(&unstaged, Stage::Unstaged));
        entries.sort_by(|a, b| a.stage.cmp(&b.stage).then_with(|| a.path.cmp(&b.path)));
        Ok(entries)
    }

    fn file_diff(&self, entry: &FileEntry) -> Result<Vec<DiffLine>, git2::Error> {
        let diff = self.diff_for_entry(entry)?;
        let mut lines = Vec::new();

        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            if lines.len() >= MAX_DIFF_LINES {
                return true;
            }

            let kind = match line.origin() {
                '+' => DiffLineKind::Addition,
                '-' => DiffLineKind::Deletion,
                'H' => DiffLineKind::HunkHeader,
                'F' => DiffLineKind::FileHeader,
                _ => DiffLineKind::Context,
            };

            let content = String::from_utf8_lossy(line.content()).to_string();
            lines.push(DiffLine { kind, content });
            true
        })?;

        if lines.len() >= MAX_DIFF_LINES {
            lines.push(DiffLine {
                kind: DiffLineKind::Context,
                content: format!("... truncated (>{MAX_DIFF_LINES} lines)"),
            });
        }

        if lines.is_empty() && entry.kind == ChangeKind::Untracked {
            lines = self.untracked_file_lines(entry);
        }

        Ok(lines)
    }
}

fn entries_from_diff(diff: &Diff<'_>, stage: Stage) -> Vec<FileEntry> {
    diff.deltas()
        .filter_map(|delta| {
            let kind = delta_to_change_kind(delta.status())?;
            let path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .unwrap_or(Path::new("unknown"))
                .to_string_lossy()
                .to_string();
            Some(FileEntry { path, kind, stage })
        })
        .collect()
}

fn delta_to_change_kind(status: Delta) -> Option<ChangeKind> {
    match status {
        Delta::Added => Some(ChangeKind::Added),
        Delta::Modified => Some(ChangeKind::Modified),
        Delta::Deleted => Some(ChangeKind::Deleted),
        Delta::Renamed => Some(ChangeKind::Renamed),
        Delta::Copied => Some(ChangeKind::Copied),
        Delta::Typechange => Some(ChangeKind::Typechange),
        Delta::Untracked => Some(ChangeKind::Untracked),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{ChangeKind, DiffLineKind, DiffSource, FileEntry, GitRepo, MAX_DIFF_LINES, Stage};
    use git2::{Repository, Signature};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// テストごとの使い捨てリポジトリ。drop で消える。
    ///
    /// 本体は git2 の読み取り系 API しか使わない方針だが、これはテストの
    /// 前提を組み立てるためだけの書き込みで、利用者のリポジトリには触れない。
    struct Fixture {
        dir: PathBuf,
        repo: Repository,
    }

    impl Fixture {
        fn new() -> Self {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "gdf-git-test-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&dir);
            let repo = Repository::init(&dir).unwrap();
            Self { dir, repo }
        }

        fn write(&self, path: &str, content: &str) -> &Self {
            self.write_bytes(path, content.as_bytes())
        }

        fn write_bytes(&self, path: &str, content: &[u8]) -> &Self {
            let full = self.dir.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, content).unwrap();
            self
        }

        fn remove(&self, path: &str) -> &Self {
            std::fs::remove_file(self.dir.join(path)).unwrap();
            self
        }

        fn stage(&self, path: &str) -> &Self {
            let mut index = self.repo.index().unwrap();
            if self.dir.join(path).exists() {
                index.add_path(Path::new(path)).unwrap();
            } else {
                index.remove_path(Path::new(path)).unwrap();
            }
            index.write().unwrap();
            self
        }

        fn commit(&self) -> &Self {
            let mut index = self.repo.index().unwrap();
            let tree = self.repo.find_tree(index.write_tree().unwrap()).unwrap();
            let signature = Signature::now("test", "test@example.com").unwrap();
            let parent = self.repo.head().ok().map(|h| h.peel_to_commit().unwrap());
            let parents: Vec<_> = parent.iter().collect();
            self.repo
                .commit(
                    Some("HEAD"),
                    &signature,
                    &signature,
                    "commit",
                    &tree,
                    &parents,
                )
                .unwrap();
            self
        }

        fn git_repo(&self) -> GitRepo {
            GitRepo::open(Some(self.dir.to_str().unwrap())).unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn listing(repo: &GitRepo) -> Vec<(String, ChangeKind, Stage)> {
        repo.changed_files()
            .unwrap()
            .into_iter()
            .map(|e| (e.path, e.kind, e.stage))
            .collect()
    }

    fn diff_of(
        repo: &GitRepo,
        path: &str,
        kind: ChangeKind,
        stage: Stage,
    ) -> Vec<(DiffLineKind, String)> {
        let entry = FileEntry {
            path: path.to_string(),
            kind,
            stage,
        };
        repo.file_diff(&entry)
            .unwrap()
            .into_iter()
            .map(|l| (l.kind, l.content))
            .collect()
    }

    fn row(path: &str, kind: ChangeKind, stage: Stage) -> (String, ChangeKind, Stage) {
        (path.to_string(), kind, stage)
    }

    /// staged を先に、同じステージの中ではパス順に並べる。
    #[test]
    fn lists_staged_then_unstaged_each_sorted_by_path() {
        let f = Fixture::new();
        f.write("a.txt", "a\n")
            .write("b.txt", "b\n")
            .write("c.txt", "c\n");
        f.stage("a.txt").stage("b.txt").stage("c.txt").commit();
        f.write("b.txt", "b2\n").stage("b.txt");
        f.remove("c.txt").stage("c.txt");
        f.write("new.txt", "n\n").stage("new.txt");
        f.write("a.txt", "a2\n");
        f.write("z.txt", "z\n");

        assert_eq!(
            listing(&f.git_repo()),
            vec![
                row("b.txt", ChangeKind::Modified, Stage::Staged),
                row("c.txt", ChangeKind::Deleted, Stage::Staged),
                row("new.txt", ChangeKind::Added, Stage::Staged),
                row("a.txt", ChangeKind::Modified, Stage::Unstaged),
                row("z.txt", ChangeKind::Untracked, Stage::Unstaged),
            ]
        );
    }

    #[test]
    fn lists_same_path_in_both_stages() {
        let f = Fixture::new();
        f.write("a.txt", "1\n").stage("a.txt").commit();
        f.write("a.txt", "2\n").stage("a.txt");
        f.write("a.txt", "3\n");

        assert_eq!(
            listing(&f.git_repo()),
            vec![
                row("a.txt", ChangeKind::Modified, Stage::Staged),
                row("a.txt", ChangeKind::Modified, Stage::Unstaged),
            ]
        );
    }

    /// 未追跡のディレクトリは、中のファイルではなくディレクトリ 1 行として出る。
    #[test]
    fn lists_untracked_directory_as_one_entry() {
        let f = Fixture::new();
        f.write("a.txt", "a\n").stage("a.txt").commit();
        f.write("dir/x.txt", "x\n").write("dir/y.txt", "y\n");

        assert_eq!(
            listing(&f.git_repo()),
            vec![row("dir/", ChangeKind::Untracked, Stage::Unstaged)]
        );
    }

    #[test]
    fn lists_nothing_for_clean_tree() {
        let f = Fixture::new();
        f.write("a.txt", "a\n").stage("a.txt").commit();

        assert_eq!(listing(&f.git_repo()), vec![]);
    }

    /// コミットが 1 つも無いリポジトリでは、空のツリーとの差分として扱う。
    #[test]
    fn handles_repository_without_commits() {
        let f = Fixture::new();
        f.write("x.txt", "x\n").stage("x.txt");
        f.write("y.txt", "y\n");
        let repo = f.git_repo();

        assert_eq!(
            listing(&repo),
            vec![
                row("x.txt", ChangeKind::Added, Stage::Staged),
                row("y.txt", ChangeKind::Untracked, Stage::Unstaged),
            ]
        );
        let diff = diff_of(&repo, "x.txt", ChangeKind::Added, Stage::Staged);
        assert_eq!(diff[0].0, DiffLineKind::FileHeader);
        assert!(
            diff[0].1.starts_with("diff --git a/x.txt b/x.txt\n"),
            "{diff:?}"
        );
        assert_eq!(
            diff[1..],
            [
                (DiffLineKind::HunkHeader, "@@ -0,0 +1 @@\n".to_string()),
                (DiffLineKind::Addition, "x\n".to_string()),
            ]
        );
    }

    #[test]
    fn reads_unstaged_and_staged_diffs_separately() {
        let f = Fixture::new();
        f.write("a.txt", "keep\nold\n").stage("a.txt").commit();
        f.write("a.txt", "keep\nstaged\n").stage("a.txt");
        f.write("a.txt", "keep\nworking\n");
        let repo = f.git_repo();

        let staged = diff_of(&repo, "a.txt", ChangeKind::Modified, Stage::Staged);
        let unstaged = diff_of(&repo, "a.txt", ChangeKind::Modified, Stage::Unstaged);

        assert_eq!(staged[0].0, DiffLineKind::FileHeader);
        assert_eq!(
            staged[1..],
            [
                (DiffLineKind::HunkHeader, "@@ -1,2 +1,2 @@\n".to_string()),
                (DiffLineKind::Context, "keep\n".to_string()),
                (DiffLineKind::Deletion, "old\n".to_string()),
                (DiffLineKind::Addition, "staged\n".to_string()),
            ]
        );
        assert_eq!(unstaged[0].0, DiffLineKind::FileHeader);
        assert_eq!(
            unstaged[1..],
            [
                (DiffLineKind::HunkHeader, "@@ -1,2 +1,2 @@\n".to_string()),
                (DiffLineKind::Context, "keep\n".to_string()),
                (DiffLineKind::Deletion, "staged\n".to_string()),
                (DiffLineKind::Addition, "working\n".to_string()),
            ]
        );
    }

    #[test]
    fn reads_staged_deletion() {
        let f = Fixture::new();
        f.write("a.txt", "gone\n").stage("a.txt").commit();
        f.remove("a.txt").stage("a.txt");

        let diff = diff_of(&f.git_repo(), "a.txt", ChangeKind::Deleted, Stage::Staged);

        assert_eq!(
            diff[1..],
            [
                (DiffLineKind::HunkHeader, "@@ -1 +0,0 @@\n".to_string()),
                (DiffLineKind::Deletion, "gone\n".to_string()),
            ]
        );
    }

    /// 未追跡ファイルは git の差分に中身が載らないので、ファイルを読んで
    /// 全行を追加行として見せる。
    #[test]
    fn shows_untracked_file_as_all_additions() {
        let f = Fixture::new();
        f.write("a.txt", "a\n").stage("a.txt").commit();
        f.write("new.txt", "one\ntwo\n");

        let diff = diff_of(
            &f.git_repo(),
            "new.txt",
            ChangeKind::Untracked,
            Stage::Unstaged,
        );

        assert_eq!(
            diff,
            [
                (DiffLineKind::FileHeader, "new file: new.txt\n".to_string()),
                (DiffLineKind::Addition, "one\n".to_string()),
                (DiffLineKind::Addition, "two\n".to_string()),
            ]
        );
    }

    #[test]
    fn shows_placeholder_for_unreadable_untracked_entries() {
        let f = Fixture::new();
        f.write("a.txt", "a\n").stage("a.txt").commit();
        f.write_bytes("bin.dat", &[0xff, 0xfe, 0x00, 0x80]);
        f.write("dir/x.txt", "x\n");
        let repo = f.git_repo();
        let placeholder = vec![(
            DiffLineKind::Context,
            "Binary file or unreadable".to_string(),
        )];

        assert_eq!(
            diff_of(&repo, "bin.dat", ChangeKind::Untracked, Stage::Unstaged),
            placeholder
        );
        assert_eq!(
            diff_of(&repo, "dir/", ChangeKind::Untracked, Stage::Unstaged),
            placeholder
        );
    }

    /// バイナリは本文の代わりに git の「differ」行が文脈行として出る。
    #[test]
    fn shows_binary_tracked_change_as_differ_line() {
        let f = Fixture::new();
        f.write_bytes("bin.dat", &[0, 1, 2, 3])
            .stage("bin.dat")
            .commit();
        f.write_bytes("bin.dat", &[0, 1, 2, 4]);

        let diff = diff_of(
            &f.git_repo(),
            "bin.dat",
            ChangeKind::Modified,
            Stage::Unstaged,
        );

        assert_eq!(diff[0].0, DiffLineKind::FileHeader);
        assert_eq!(
            diff[1..],
            [(
                DiffLineKind::Context,
                "Binary files a/bin.dat and b/bin.dat differ\n".to_string()
            )]
        );
    }

    /// 追跡済みファイルは上限で打ち切り、末尾にその旨を足す。
    #[test]
    fn truncates_long_tracked_diff_with_notice() {
        let f = Fixture::new();
        f.write("a.txt", "").stage("a.txt").commit();
        f.write("a.txt", &"line\n".repeat(MAX_DIFF_LINES + 5));

        let diff = diff_of(
            &f.git_repo(),
            "a.txt",
            ChangeKind::Modified,
            Stage::Unstaged,
        );

        assert_eq!(diff.len(), MAX_DIFF_LINES + 1);
        assert_eq!(
            diff.last().unwrap(),
            &(
                DiffLineKind::Context,
                format!("... truncated (>{MAX_DIFF_LINES} lines)")
            )
        );
    }

    /// 未追跡ファイルも本文を上限で打ち切るが、打ち切った旨は足さない。
    #[test]
    fn truncates_long_untracked_file_without_notice() {
        let f = Fixture::new();
        f.write("a.txt", "a\n").stage("a.txt").commit();
        f.write("new.txt", &"line\n".repeat(MAX_DIFF_LINES + 5));

        let diff = diff_of(
            &f.git_repo(),
            "new.txt",
            ChangeKind::Untracked,
            Stage::Unstaged,
        );

        assert_eq!(diff.len(), 1 + MAX_DIFF_LINES);
        assert_eq!(
            diff.last().unwrap(),
            &(DiffLineKind::Addition, "line\n".to_string())
        );
    }
}
