use crate::github::PullRequest;
use std::collections::HashMap;

/// Position of a pull request within a stack.
///
/// Only PRs that are part of a chain of two or more PRs get a `StackPosition`.
/// Standalone PRs (not stacked) are absent from the map returned by
/// [`compute_stack_positions`].
#[derive(Debug, Clone)]
pub struct StackPosition {
    /// PR number of the bottom-most PR in this stack (the one targeting trunk).
    /// Used as a stable identifier for the stack as a whole.
    pub stack_id: u64,
    /// Depth within the stack: 0 = bottom (closest to trunk), 1 = one above, etc.
    pub depth: usize,
    /// Total number of PRs in this stack.
    pub total: usize,
    /// PR number of the PR immediately below this one (None if this is the bottom).
    pub below: Option<u64>,
    /// PR number of the PR immediately above this one (None if this is the top).
    pub above: Option<u64>,
}

/// Infer stack structure from a list of open pull requests.
///
/// A PR B is considered stacked on PR A when `B.base_branch == A.head_branch`.
/// This works with any stacking tool (gh-stack, git-town, manual) because the
/// relationship is encoded in the standard PR base/head branches — no private
/// GitHub API is needed.
///
/// Returns a map from `pr_number` → `StackPosition` for every PR that is part
/// of a chain. Standalone (non-stacked) PRs are not included in the result.
pub fn compute_stack_positions(prs: &[PullRequest]) -> HashMap<u64, StackPosition> {
    if prs.is_empty() {
        return HashMap::new();
    }

    // Index: head_branch → pr_number
    // Used to check if a PR's base_branch matches some other PR's head_branch
    // (which would mean this PR is NOT the bottom of the stack).
    let head_to_number: HashMap<&str, u64> = prs
        .iter()
        .map(|pr| (pr.head_branch.as_str(), pr.number))
        .collect();

    // Index: base_branch → pr_number
    // Used to walk upward: given the current PR's head_branch, find the PR
    // whose base_branch equals that head_branch (i.e. the PR stacked on top).
    let base_to_number: HashMap<&str, u64> = prs
        .iter()
        .map(|pr| (pr.base_branch.as_str(), pr.number))
        .collect();

    // Index: pr_number → &PullRequest for quick lookup
    let by_number: HashMap<u64, &PullRequest> = prs.iter().map(|pr| (pr.number, pr)).collect();

    let mut positions: HashMap<u64, StackPosition> = HashMap::new();

    for pr in prs {
        // A PR is the bottom of a stack when its base_branch does NOT match any
        // other PR's head_branch (i.e. it targets trunk/main, not another PR's branch).
        let is_bottom = !head_to_number.contains_key(pr.base_branch.as_str());
        if !is_bottom {
            continue;
        }

        // Walk the chain upward from this PR, collecting all members.
        // "Above" means: find the PR whose base_branch == my head_branch.
        let mut chain: Vec<u64> = Vec::new();
        let mut current_number = pr.number;

        loop {
            chain.push(current_number);

            let current_head = by_number
                .get(&current_number)
                .map(|p| p.head_branch.as_str())
                .unwrap_or("");

            // Find the PR whose base_branch == current PR's head_branch.
            match base_to_number.get(current_head) {
                Some(&above_number) if above_number != current_number => {
                    current_number = above_number;
                }
                _ => break,
            }
        }

        // Only record positions for chains of length >= 2.
        if chain.len() < 2 {
            continue;
        }

        let total = chain.len();
        let stack_id = chain[0]; // bottom PR number

        for (depth, &number) in chain.iter().enumerate() {
            let below = if depth > 0 {
                Some(chain[depth - 1])
            } else {
                None
            };
            let above = chain.get(depth + 1).copied();
            positions.insert(
                number,
                StackPosition {
                    stack_id,
                    depth,
                    total,
                    below,
                    above,
                },
            );
        }
    }

    positions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::PullRequest;

    fn make_pr(number: u64, head: &str, base: &str) -> PullRequest {
        PullRequest {
            number,
            title: format!("PR {number}"),
            author: "alice".to_string(),
            head_branch: head.to_string(),
            base_branch: base.to_string(),
            head_sha: String::new(),
            base_sha: String::new(),
            state: "open".to_string(),
            draft: false,
            url: String::new(),
            additions: None,
            deletions: None,
            changed_files: None,
            review_decision: None,
            ci_status: None,
            body: None,
        }
    }

    #[test]
    fn test_no_stack() {
        let prs = vec![
            make_pr(1, "feat/foo", "main"),
            make_pr(2, "feat/bar", "main"),
        ];
        let result = compute_stack_positions(&prs);
        assert!(result.is_empty(), "standalone PRs should not be in the map");
    }

    #[test]
    fn test_two_pr_stack() {
        let prs = vec![
            make_pr(1, "feat/auth", "main"),
            make_pr(2, "feat/api", "feat/auth"),
        ];
        let result = compute_stack_positions(&prs);
        assert_eq!(result.len(), 2);

        let bottom = result.get(&1).unwrap();
        assert_eq!(bottom.depth, 0);
        assert_eq!(bottom.total, 2);
        assert_eq!(bottom.stack_id, 1);
        assert_eq!(bottom.below, None);
        assert_eq!(bottom.above, Some(2));

        let top = result.get(&2).unwrap();
        assert_eq!(top.depth, 1);
        assert_eq!(top.total, 2);
        assert_eq!(top.stack_id, 1);
        assert_eq!(top.below, Some(1));
        assert_eq!(top.above, None);
    }

    #[test]
    fn test_three_pr_stack() {
        let prs = vec![
            make_pr(10, "feat/auth", "main"),
            make_pr(11, "feat/api", "feat/auth"),
            make_pr(12, "feat/ui", "feat/api"),
        ];
        let result = compute_stack_positions(&prs);
        assert_eq!(result.len(), 3);

        let mid = result.get(&11).unwrap();
        assert_eq!(mid.depth, 1);
        assert_eq!(mid.total, 3);
        assert_eq!(mid.below, Some(10));
        assert_eq!(mid.above, Some(12));
    }

    #[test]
    fn test_mixed_stacked_and_standalone() {
        let prs = vec![
            make_pr(1, "feat/auth", "main"),
            make_pr(2, "feat/api", "feat/auth"),
            make_pr(3, "feat/unrelated", "main"), // standalone
        ];
        let result = compute_stack_positions(&prs);
        assert_eq!(result.len(), 2);
        assert!(result.contains_key(&1));
        assert!(result.contains_key(&2));
        assert!(!result.contains_key(&3));
    }
}
