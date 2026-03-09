# kb

Simple, opinionated kanban tui.

## Default config

`kb` reads config from `~/.kb/config.toml`.
If the file does not exist, the defaults below are used.

```toml
[database]
# Override if you want a custom DB location.
# Default (macOS): ~/Library/Application Support/kb/kanban.db
# Default (Linux): ~/.local/share/kb/kanban.db
# path = "~/Library/Application Support/kb/kanban.db"

[limits]
today_hard_limit = 4
this_week_soft_limit = 10

[colors]
bg = "reset"
fg = "gray"
border = "darkgray"
active_border = "cyan"
due_overdue = "red"
due_today = "yellow"
due_soon = "cyan"
title = "white"
```

Color values accept named colors or hex (e.g., `"#1f232b"`).
