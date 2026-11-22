# Quinjet over Graftcode: browser frontend

A TypeScript page that lists a repository's changed files by calling
`Quinjet.status` through Quinjet's generated npm Graft.

```bash
quinjet graft serve --publish
npm install
npm run graft
npm run dev
```

Run the first command in the repository to show, the rest here, then open
<http://localhost:5173>. The [walkthrough](../../docs/guides/graftcode-demo.md)
explains each step.
