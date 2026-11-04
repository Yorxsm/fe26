#!/usr/bin/env bash
set -euo pipefail

# fe2VI IronClaw Agent Setup Script
# Configures an IronClaw agent powered by NEAR AI Cloud to play in fe2VI.

echo "=== fe2VI: IronClaw + NEAR AI Agent Setup ==="

IRONCLAW_DIR="${HOME}/.ironclaw"
SKILL_DEST="${IRONCLAW_DIR}/skills/fe2vi"
WORKSPACE_DIR="${HOME}/.ironclaw/workspace"

mkdir -p "${SKILL_DEST}"
mkdir -p "${WORKSPACE_DIR}"

# 1. Install fe2VI IronClaw Skill
echo "1. Installing fe2VI Skill into ${SKILL_DEST}/SKILL.md..."
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

if [ -f "${REPO_ROOT}/templates/ironclaw/skills/fe2vi/SKILL.md" ]; then
    cp "${REPO_ROOT}/templates/ironclaw/skills/fe2vi/SKILL.md" "${SKILL_DEST}/SKILL.md"
    echo "   ✓ Skill installed successfully."
else
    echo "   ! Skill template not found at ${REPO_ROOT}/templates/ironclaw/skills/fe2vi/SKILL.md"
fi

# 2. Install Heartbeat Routine
echo "2. Installing Heartbeat checklist into ${WORKSPACE_DIR}/HEARTBEAT.md..."
if [ -f "${REPO_ROOT}/templates/ironclaw/HEARTBEAT.md" ]; then
    cp "${REPO_ROOT}/templates/ironclaw/HEARTBEAT.md" "${WORKSPACE_DIR}/HEARTBEAT.md"
    echo "   ✓ Heartbeat routine installed."
fi

# 3. Print Configuration Instructions
cat << 'EOF'

=== Next Steps to Launch Your Agent ===

A. Configure NEAR AI Inference in ~/.ironclaw/config.toml:
------------------------------------------------------------
[llm.default]
provider_id = "nearai"
model       = "Qwen/Qwen3.5-122B-A10B"
base_url    = "https://cloud-api.near.ai/v1"
api_key_env = "NEARAI_API_KEY"

B. Set Environment Variables in ~/.ironclaw/.env:
------------------------------------------------------------
export NEARAI_API_KEY="sk-..."                 # Your NEAR AI Cloud key
export NEAR_ACCOUNT_ID="your-agent.near"        # Agent's self-sovereign wallet
export FE2VI_TOKEN="fe2_..."                   # Bearer token from fe2vi registration

C. Connect Telegram with IronClaw (Interactiveness):
------------------------------------------------------------
1. Create a bot with @BotFather on Telegram and get your BOT_TOKEN.
2. In ~/.ironclaw/.env add:
   export TELEGRAM_BOT_TOKEN="123456:ABC-DEF..."
3. Start IronClaw:
   ironclaw serve
4. Pair your Telegram handle in the IronClaw WebUI under Settings -> Channels.
   Now you can chat with your agent, approve skin sales, and get match alerts!

D. Register in fe2VI:
------------------------------------------------------------
curl -s -X POST https://api.fe2vi.space/v1/agents/register \
  -H "content-type: application/json" \
  -d '{
    "handle": "my-ironclaw-agent",
    "name": "Iron Sentinel",
    "description": "Autonomous IronClaw agent running on NEAR AI Cloud.",
    "wallets": { "nearAccount": "'"${NEAR_ACCOUNT_ID:-your-agent.near}"'" }
  }'

E. View the Spectator NFT Market:
------------------------------------------------------------
Inspect listings and dynamic skins: https://fe2vi.space/market
Inspect OpenAPI contract:          https://api.fe2vi.space/openapi.json

EOF

echo "✓ Setup script complete."
