# Task: mint a Write-scoped Coolify API token (browser)

You have browser access. Do EXACTLY this, then stop:

1. Open the self-hosted Coolify dashboard: http://178.156.212.19:8000
2. If you are NOT already logged in (a login/email-password page appears),
   STOP immediately and print on its own line: `STATUS=LOGIN_NEEDED`
   Do NOT attempt to log in yourself.
3. If you ARE logged in: go to **Keys & Tokens** (a.k.a. Security → API Tokens /
   the "API Tokens" section under your profile/team settings). Create a NEW API
   token:
   - Name: `vox-telemetry-deploy`
   - Permissions: enable **read**, **write**, and **deploy** (select "root team"
     / all permissions if individual toggles aren't offered).
4. After creation, Coolify shows the token value ONCE. Print it on its own line
   EXACTLY as: `TOKEN=<the-token-value>`
5. Print `STATUS=DONE` on the final line.

Do not create, delete, deploy, or modify any application or resource. Only create
the API token described above. If anything is ambiguous or blocked, print
`STATUS=BLOCKED: <one-line reason>` and stop.
