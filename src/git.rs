use git2::{Delta, Diff, DiffOptions, Repository};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    pub fn changed_files(&self) -> Result<Vec<FileEntry>, git2::Error> {
        let mut entries = Vec::new();

        // staged changes: HEAD tree vs index
        let head_tree = self
            .repo
            .head()
            .and_then(|r| r.peel_to_tree())
            .ok();

        let staged_diff = self.repo.diff_tree_to_index(
            head_tree.as_ref(),
            None,
            Some(&mut DiffOptions::new()),
        )?;

        for delta in staged_diff.deltas() {
            let path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .unwrap_or(Path::new("unknown"))
                .to_string_lossy()
                .to_string();

            if let Some(kind) = delta_to_change_kind(delta.status()) {
                entries.push(FileEntry {
                    path,
                    kind,
                    stage: Stage::Staged,
                });
            }
        }

        // unstaged changes: index vs workdir
        let unstaged_diff = self.repo.diff_index_to_workdir(
            None,
            Some(DiffOptions::new().include_untracked(true)),
        )?;

        for delta in unstaged_diff.deltas() {
            let path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .unwrap_or(Path::new("unknown"))
                .to_string_lossy()
                .to_string();

            if let Some(kind) = delta_to_change_kind(delta.status()) {
                entries.push(FileEntry {
                    path,
                    kind,
                    stage: Stage::Unstaged,
                });
            }
        }

        // sort: staged first, then by path
        entries.sort_by(|a, b| {
            a.stage
                .cmp_staged_first(&b.stage)
                .then_with(|| a.path.cmp(&b.path))
        });

        Ok(entries)
    }

    pub fn file_diff(&self, entry: &FileEntry) -> Result<Vec<DiffLine>, git2::Error> {
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

    fn diff_for_entry(&self, entry: &FileEntry) -> Result<Diff<'_>, git2::Error> {
        let mut opts = DiffOptions::new();
        opts.pathspec(&entry.path);
        opts.include_untracked(true);
        opts.recurse_untracked_dirs(true);

        match entry.stage {
            Stage::Staged => {
                let head_tree = self.repo.head().and_then(|r| r.peel_to_tree()).ok();
                self.repo
                    .diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))
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

impl Stage {
    fn cmp_staged_first(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Stage::Staged, Stage::Unstaged) => std::cmp::Ordering::Less,
            (Stage::Unstaged, Stage::Staged) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        }
    }
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
