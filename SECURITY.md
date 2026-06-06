# Security

## Reporting

Report security issues privately to the repository maintainer (do not open public issues for sensitive vulnerabilities).

## Known risks (planned features)

- **SSH passwords:** storing or transmitting credentials incorrectly could expose remote hosts.
- **DPAPI session files:** `%LocalAppData%\ScriptReloader\session.json` will hold encrypted session data; treat the directory as sensitive on shared machines.

## Policy

- Never commit passwords, private keys, or session tokens.
- Use `.env` / local config (gitignored) for development secrets.
- Review SSH and session code carefully before release.
