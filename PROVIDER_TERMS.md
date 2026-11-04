# Provider terms and permitted use

Official terms reviewed on 4 November 2026. Check current terms and any agreement specific
to your organization, region and provider before use.

rightcommit is an independent self-hosted orchestration tool for individual and internal
organizational use. The documented scenarios use API keys or authorized provider
configuration through environment variables. See [Intended use](INTENDED_USE.md).
The original agent CLIs are installed and run without modifying their binaries.

## Anthropic and Claude Code

Preinstalling or running Claude Code inside a product or service requires agreement to
Anthropic's [Commercial Terms](https://www.anthropic.com/legal/commercial-terms), unless
otherwise agreed, and compliance with the conditions in
[Claude Code legal and compliance](https://code.claude.com/docs/en/legal-and-compliance).
Keep the original binary and its built-in authentication methods intact. Product references
must not imply Anthropic endorsement or affiliation.

Anthropic expressly permits organizational management of API/cloud credentials for the
customer's own authorized users, billed to the credential owner under their agreement.
An internal deployment is distinct from offering Claude Code to external customers; the
latter has additional credential, direct-billing and non-intermediation conditions.
Follow the [Usage Policy](https://www.anthropic.com/legal/aup) and any cloud-provider agreement.

## OpenAI and Codex

OpenAI's [Services Agreement](https://openai.com/policies/services-agreement/), sections 2.2
and 3.2, allows API-based customer applications serving end users. An internal deployment
can use appropriately scoped organizational API credentials under its agreement. Do not
share individual account credentials, transfer keys to third parties, resell account access
or bypass usage limits. Review the incorporated policies and service-specific terms.
Use API authentication for programmatic Codex CLI workflows, as described in the
[official authentication documentation](https://learn.chatgpt.com/docs/auth?surface=cli).

## Pi, other providers and data

Use API credentials authorized for the selected provider and users. OpenRouter usage is
subject to its [Terms of Service](https://openrouter.ai/terms) and applicable model terms.
Cloud services require their own agreements. Local servers and models retain their software
and model licenses; API compatibility does not grant provider access or imply affiliation.

Only send data you are entitled to process. Review provider retention, privacy and training
settings, including required workplace/customer authorizations. Protect `.env`, user homes,
the encrypted database, master key and backups; revoke exposed credentials.

These references do not accept agreements on anyone's behalf or certify compliance.
Provider agreements are separate from the rightcommit [license](LICENSE).
