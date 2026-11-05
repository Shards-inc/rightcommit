# Intended use and third-party provider obligations

rightcommit's documented and supported deployment scope is personal use or **internal use
by an organization and its authorized users**. It is not offered as an intermediary,
provider-access reseller or shared credential service for external users. Do not use
this deployment model to sell access to an AI provider, rebill provider consumption to
external users, or let unrelated users use the operator's provider credentials.

The products and services integrated by rightcommit retain their own licenses, agreements
and usage policies. Installing rightcommit, building its container locally, or configuring
an API key does not waive those requirements or grant additional provider rights. The
operator and each user must comply with the terms applicable to their role and use:

- **Anthropic / Claude Code:** comply with the applicable Commercial Terms and the
  conditions for integrating Claude Code; keep the original binary and its built-in
  authentication mechanisms intact. Organizational API/cloud credentials may serve the
  organization's own authorized users, with usage billed to the credential owner under
  its agreement. Do not resell or intermediate Claude Code usage for external users.
  See [Claude Code legal and compliance](https://code.claude.com/docs/en/legal-and-compliance).
- **OpenAI / Codex:** comply with the applicable Services Agreement and incorporated
  policies. Keep individual account credentials separate; do not resell account access,
  transfer API keys to third parties or bypass usage limits. API application permissions
  do not remove these restrictions. See the
  [OpenAI Services Agreement](https://openai.com/policies/services-agreement/).
- **Pi and other providers:** Pi does not grant rights to the model services it connects
  to. Follow the selected provider's agreement, credential permissions, billing conditions
  and model terms, including [OpenRouter's terms](https://openrouter.ai/terms) when selected.
  Cloud services and local models retain their own applicable agreements and licenses.
- **Data and third-party material:** process only data you are authorized to send, observe
  the selected service's privacy and retention requirements, and preserve required
  software license notices and trademark attribution. Provider names do not imply endorsement.

These are provider-specific obligations, not a claim that every provider has identical
terms. See [Provider terms and permitted use](PROVIDER_TERMS.md). rightcommit's own license
is separate from provider agreements; it does not authorize otherwise restricted provider
access. This documentation does not accept agreements on anyone's behalf or certify compliance.
