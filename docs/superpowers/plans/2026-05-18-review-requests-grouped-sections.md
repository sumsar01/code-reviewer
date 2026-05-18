# Review Requests Grouped Sections Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the flat Review Requests list with Option A — two grouped sections: "NEEDS YOUR REVIEW" (action items) and "WAITING FOR AUTHOR" (already reviewed, dimmed).

**Architecture:** All logic lives in `src/ui/review_requests.rs`. Split `app.rr_prs` into two groups by `review_decision` at render time; build a flat `display_items` vec of `DisplayItem` (either a section header or a PR row) and map `rr_cursor` into it, skipping headers.

**Tech Stack:** Rust, Ratatui (TUI), existing `ReviewDecision` enum from `src/github.rs`.

---

### Task 1: Add grouped rendering to `render_list`

**Files:**
- Modify: `src/ui/review_requests.rs`

The key change: instead of iterating `app.rr_prs` directly, split into two groups and insert section header rows. The `rr_cursor` already points into `app.rr_prs` by index; we need a mapping from display position → pr index and back, skipping header rows for selection highlight.

- [ ] **Step 1: Read current file to have it in memory**

Open `src/ui/review_requests.rs` (already read — 306 lines).

- [ ] **Step 2: Add a `DisplayItem` enum and `build_display_items` helper**

Add near the top of `render_list`, before the `items` build loop:

```rust
enum DisplayItem<'a> {
    SectionHeader { label: &'static str, count: usize, dimmed: bool },
    PrRow { pr: &'a crate::github::ReviewRequestPr, pr_index: usize, dimmed: bool },
}
```

Then add a helper function (outside `render_list`, after `truncate`):

```rust
fn build_display_items(prs: &[crate::github::ReviewRequestPr]) -> Vec<DisplayItem> {
    let needs: Vec<(usize, &crate::github::ReviewRequestPr)> = prs
        .iter()
        .enumerate()
        .filter(|(_, pr)| matches!(
            pr.review_decision,
            ReviewDecision::ReviewRequired | ReviewDecision::Unknown
        ))
        .collect();
    let waiting: Vec<(usize, &crate::github::ReviewRequestPr)> = prs
        .iter()
        .enumerate()
        .filter(|(_, pr)| matches!(
            pr.review_decision,
            ReviewDecision::Approved | ReviewDecision::ChangesRequested
        ))
        .collect();

    let mut items: Vec<DisplayItem> = Vec::new();

    items.push(DisplayItem::SectionHeader {
        label: "NEEDS YOUR REVIEW",
        count: needs.len(),
        dimmed: false,
    });
    for (idx, pr) in &needs {
        items.push(DisplayItem::PrRow { pr, pr_index: *idx, dimmed: false });
    }

    if !waiting.is_empty() {
        items.push(DisplayItem::SectionHeader {
            label: "WAITING FOR AUTHOR",
            count: waiting.len(),
            dimmed: true,
        });
        for (idx, pr) in &waiting {
            items.push(DisplayItem::PrRow { pr, pr_index: *idx, dimmed: true });
        }
    }

    items
}
```

- [ ] **Step 3: Build a cursor mapping helper**

The `rr_cursor` is an index into `app.rr_prs`. We need to find which display position corresponds to a given `pr_index`. Add this helper:

```rust
fn display_index_for_pr(display_items: &[DisplayItem], pr_index: usize) -> Option<usize> {
    display_items.iter().position(|item| {
        matches!(item, DisplayItem::PrRow { pr_index: idx, .. } if *idx == pr_index)
    })
}
```

- [ ] **Step 4: Rewrite `render_list` body to use grouped items**

Replace the entire `items: Vec<ListItem>` build block (lines 181–287) with:

```rust
    let display = build_display_items(&app.rr_prs);

    // Fixed columns total (excluding title).
    let fixed_cols = 1  // leading space
        + COL_REPO_WIDTH + COL_GAP
        + COL_NUMBER_WIDTH + COL_GAP
        + COL_REVIEW_WIDTH + COL_GAP
        + COL_CI_WIDTH + COL_GAP
        + COL_UPDATED_WIDTH + COL_GAP
        + COL_AUTHOR_WIDTH;

    let items: Vec<ListItem> = display
        .iter()
        .map(|item| match item {
            DisplayItem::SectionHeader { label, count, dimmed } => {
                let style = if *dimmed {
                    Style::default().fg(t.text_dim)
                } else {
                    Style::default()
                        .fg(t.text_accent)
                        .add_modifier(Modifier::BOLD)
                };
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!(" ▼ {} ({})", label, count),
                        style,
                    ),
                ]))
            }
            DisplayItem::PrRow { pr, dimmed, .. } => {
                let dim_style = if *dimmed {
                    Style::default().fg(t.text_dim)
                } else {
                    Style::default()
                };

                let repo = truncate(
                    &format!("{}/{}", pr.repo_owner, pr.repo_name),
                    COL_REPO_WIDTH,
                );
                let repo_col = format!("{:<width$}", repo, width = COL_REPO_WIDTH);
                let number = format!("#{:<width$}", pr.number, width = COL_NUMBER_WIDTH - 1);
                let updated = format!("{:<width$}", pr.updated_at, width = COL_UPDATED_WIDTH);
                let author = format!("{:<width$}", pr.author, width = COL_AUTHOR_WIDTH);

                let inner_width = area.width.saturating_sub(2) as usize;
                let draft_extra = if pr.draft { COL_DRAFT_WIDTH + COL_GAP } else { 0 };
                let title_width = inner_width
                    .saturating_sub(fixed_cols + draft_extra + COL_GAP);
                let title_display = format!(
                    "{:<width$}",
                    truncate(&pr.title, title_width),
                    width = title_width
                );

                // Review decision badge
                let (review_text, review_style) = match &pr.review_decision {
                    ReviewDecision::Approved => (
                        format!("{:<width$}", "✓ Approved", width = COL_REVIEW_WIDTH),
                        if *dimmed {
                            Style::default().fg(t.text_dim)
                        } else {
                            Style::default().fg(t.diff_added_fg).add_modifier(Modifier::BOLD)
                        },
                    ),
                    ReviewDecision::ChangesRequested => (
                        format!("{:<width$}", "✗ Changes", width = COL_REVIEW_WIDTH),
                        if *dimmed {
                            Style::default().fg(t.text_dim)
                        } else {
                            Style::default().fg(t.diff_removed_fg).add_modifier(Modifier::BOLD)
                        },
                    ),
                    ReviewDecision::ReviewRequired | ReviewDecision::Unknown => (
                        format!("{:<width$}", "⏳ Waiting", width = COL_REVIEW_WIDTH),
                        Style::default().fg(t.pr_number).add_modifier(Modifier::BOLD),
                    ),
                };

                // CI status badge
                let (ci_text, ci_style) = match &pr.ci_status {
                    CiStatus::Success => (
                        format!("{:<width$}", "● pass", width = COL_CI_WIDTH),
                        if *dimmed { Style::default().fg(t.text_dim) } else { Style::default().fg(t.diff_added_fg) },
                    ),
                    CiStatus::Failure => (
                        format!("{:<width$}", "✗ fail", width = COL_CI_WIDTH),
                        if *dimmed { Style::default().fg(t.text_dim) } else { Style::default().fg(t.diff_removed_fg) },
                    ),
                    CiStatus::Pending => (
                        format!("{:<width$}", "○ pend", width = COL_CI_WIDTH),
                        if *dimmed { Style::default().fg(t.text_dim) } else { Style::default().fg(t.pr_number) },
                    ),
                    CiStatus::Unknown => (
                        format!("{:<width$}", "– –", width = COL_CI_WIDTH),
                        Style::default().fg(t.text_dim),
                    ),
                };

                let mut spans = vec![
                    Span::styled("  ", Style::default()),  // indent under header
                    Span::styled(repo_col, Style::default().fg(t.text_dim)),
                    Span::styled("  ", Style::default()),
                    Span::styled(
                        number,
                        if *dimmed {
                            Style::default().fg(t.text_dim)
                        } else {
                            Style::default().fg(t.pr_number).add_modifier(Modifier::BOLD)
                        },
                    ),
                    Span::styled("  ", Style::default()),
                    Span::styled(review_text, review_style),
                    Span::styled("  ", Style::default()),
                    Span::styled(ci_text, ci_style),
                    Span::styled("  ", Style::default()),
                    Span::styled(updated, dim_style.clone()),
                    Span::styled("  ", Style::default()),
                    Span::styled(title_display, if *dimmed { Style::default().fg(t.text_dim) } else { Style::default().fg(t.text) }),
                ];

                if pr.draft {
                    spans.push(Span::styled("  ", Style::default()));
                    spans.push(Span::styled(
                        "▸DRAFT",
                        Style::default().fg(t.pr_draft).add_modifier(Modifier::BOLD),
                    ));
                }

                spans.push(Span::styled("  ", Style::default()));
                spans.push(Span::styled(author, dim_style));

                ListItem::new(Line::from(spans))
            }
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(t.border_dim_style())
                .style(t.background_style()),
        )
        .highlight_style(t.selection_style());

    let mut list_state = ListState::default();
    // Map rr_cursor (pr index) to display position (skipping headers)
    let display_pos = display_index_for_pr(&display, app.rr_cursor)
        .unwrap_or(0);
    list_state.select(Some(display_pos));

    f.render_stateful_widget(list, area, &mut list_state);
```

- [ ] **Step 5: Update cursor navigation in `src/input/review_requests.rs` to skip headers**

When user presses `j` (down) or `k` (up), the cursor must skip section header rows. Currently it just increments/decrements `rr_cursor` (which is a pr index, not a display index). Since the grouping is only visual, `rr_cursor` still moves through `app.rr_prs` by index — no change needed for the input handler. The display mapping handles it.

Verify by reading the input handler:

```bash
cat src/input/review_requests.rs
```

- [ ] **Step 6: Build and check for compile errors**

```bash
cd /Users/emtb/Developer/code-reviewer && cargo build 2>&1
```

Expected: compilation succeeds with no errors (warnings are acceptable).

- [ ] **Step 7: Commit**

```bash
cd /Users/emtb/Developer/code-reviewer && git add src/ui/review_requests.rs && git commit -m "feat: group review requests into Needs Review / Waiting for Author sections"
```
