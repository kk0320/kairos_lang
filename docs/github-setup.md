# GitHub Setup

## Recommended repository name
`kairos-lang`

## First push
```bash
git init
git add .
git commit -m "chore: initialize Kairos language repository"
git branch -M main
git remote add origin git@github.com:<YOUR_ACCOUNT>/kairos-lang.git
git push -u origin main
```

## Optional GitHub CLI
```bash
gh repo create kairos-lang --public --source=. --remote=origin --push
```

## Suggested labels
- `parser`
- `semantic`
- `ir`
- `formatter`
- `cli`
- `docs`
- `good first issue`
- `design`
