# Security

## Reporting

Report security issues privately to the repository maintainer (do not open public issues for sensitive vulnerabilities).

## Known risks (planned features)

- **SSH passwords:** storing or transmitting credentials incorrectly could expose remote hosts.
- **DPAPI session files:** `%LocalAppData%\ContainerWatch\session.json` holds encrypted session data; treat the directory as sensitive on shared machines. This path is separate from the .NET Script Reloader app.

## Policy

- Never commit passwords, private keys, or session tokens.
- Use `.env` / local config (gitignored) for development secrets.
- Review SSH and session code carefully before release.
