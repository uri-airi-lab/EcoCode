#!/usr/bin/env bash
# EcoCode — Azure Container Apps deployment script
#
# Prerequisites:
#   - Azure CLI installed and logged in  (az login)
#   - Docker running locally
#   - ./backend/.env.azure filled in (copy from .env.azure.example)
#
# Usage:
#   chmod +x deploy-azure.sh
#   ./deploy-azure.sh

set -euo pipefail

# ── Configuration ─────────────────────────────────────────────────────────────
RESOURCE_GROUP="ecocode-rg"
LOCATION="eastus"                   # policy-allowed; matches Neon us-east-1
ACR_NAME="ecocoderegistry"          # must be globally unique, lowercase, 5-50 chars
ENVIRONMENT="ecocode-env"
BACKEND_APP="backend"
FRONTEND_APP="frontend"
FRONTEND_CORS_URL="https://frontend.placeholder"

# Load secrets from file
ENV_FILE="$(dirname "$0")/backend/.env.azure"
if [[ ! -f "$ENV_FILE" ]]; then
  echo "ERROR: $ENV_FILE not found."
  echo "       Copy web/backend/.env.azure.example → web/backend/.env.azure and fill it in."
  exit 1
fi

# shellcheck disable=SC1090
source "$ENV_FILE"

: "${DATABASE_URL:?DATABASE_URL must be set in .env.azure}"
: "${JWT_SECRET:?JWT_SECRET must be set in .env.azure}"

# ── Helper ────────────────────────────────────────────────────────────────────
log() { echo; echo "▶  $*"; }

# ── 1. Resource group ─────────────────────────────────────────────────────────
log "Creating resource group '$RESOURCE_GROUP' in $LOCATION"
az group create --name "$RESOURCE_GROUP" --location "$LOCATION" --output none

# ── 2. Azure Container Registry ───────────────────────────────────────────────
log "Creating container registry '$ACR_NAME'"
az acr create \
  --name "$ACR_NAME" \
  --resource-group "$RESOURCE_GROUP" \
  --sku Basic \
  --admin-enabled true \
  --output none

log "Logging in to ACR"
az acr login --name "$ACR_NAME"

ACR_LOGIN_SERVER=$(az acr show --name "$ACR_NAME" --query loginServer -o tsv)
BACKEND_IMAGE="$ACR_LOGIN_SERVER/$BACKEND_APP:latest"
FRONTEND_IMAGE="$ACR_LOGIN_SERVER/$FRONTEND_APP:latest"

# ── 3. Build & push backend ───────────────────────────────────────────────────
log "Building backend image"
docker build -t "$BACKEND_IMAGE" "$(dirname "$0")/backend"

log "Pushing backend image"
docker push "$BACKEND_IMAGE"

# ── 4. Container Apps environment ────────────────────────────────────────────
log "Creating Container Apps environment '$ENVIRONMENT'"
az containerapp env create \
  --name "$ENVIRONMENT" \
  --resource-group "$RESOURCE_GROUP" \
  --location "$LOCATION" \
  --output none 2>/dev/null || echo "  (environment already exists, continuing)"

# ── 5. Deploy backend ─────────────────────────────────────────────────────────
log "Deploying backend container app"

ACR_USER="$(az acr credential show --name "$ACR_NAME" --query username -o tsv)"
ACR_PASS="$(az acr credential show --name "$ACR_NAME" --query 'passwords[0].value' -o tsv)"

if az containerapp show --name "$BACKEND_APP" --resource-group "$RESOURCE_GROUP" --output none 2>/dev/null; then
  # Update existing app secrets and image
  az containerapp secret set \
    --name "$BACKEND_APP" \
    --resource-group "$RESOURCE_GROUP" \
    --secrets \
        "db-url=$DATABASE_URL" \
        "jwt-secret=$JWT_SECRET" \
    --output none

  az containerapp update \
    --name "$BACKEND_APP" \
    --resource-group "$RESOURCE_GROUP" \
    --image "$BACKEND_IMAGE" \
    --output none
else
  # First-time create — set secrets, env vars, ingress
  az containerapp create \
    --name "$BACKEND_APP" \
    --resource-group "$RESOURCE_GROUP" \
    --environment "$ENVIRONMENT" \
    --image "$BACKEND_IMAGE" \
    --registry-server "$ACR_LOGIN_SERVER" \
    --registry-username "$ACR_USER" \
    --registry-password "$ACR_PASS" \
    --target-port 3001 \
    --ingress external \
    --min-replicas 1 \
    --max-replicas 3 \
    --secrets \
        "db-url=$DATABASE_URL" \
        "jwt-secret=$JWT_SECRET" \
    --env-vars \
        "DATABASE_URL=secretref:db-url" \
        "JWT_SECRET=secretref:jwt-secret" \
        "PRODUCTION=true" \
        "CORS_ORIGIN=$FRONTEND_CORS_URL" \
    --output none
fi

BACKEND_FQDN=$(az containerapp show \
  --name "$BACKEND_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --query properties.configuration.ingress.fqdn -o tsv)

BACKEND_EXTERNAL_URL="https://$BACKEND_FQDN"
# Internal URL used by the Next.js server inside the same Container Apps environment
# Container Apps backend ingress terminates traffic and exposes service internally on port 80.
BACKEND_INTERNAL_URL="http://$BACKEND_APP"

log "Backend deployed → $BACKEND_EXTERNAL_URL"

# ── 6. Build & push frontend (using internal backend URL for Next.js rewrites) ───────
# IMPORTANT: Use the internal service name for rewrites because:
#   - Next.js rewrites happen server-side (no CORS issues)
#   - Internal HTTP is faster than external HTTPS
#   - Avoids SSL/certificate/DNS resolution overhead
log "Building frontend image (NEXT_PUBLIC_API_URL=$BACKEND_INTERNAL_URL)"
docker build \
  --build-arg "NEXT_PUBLIC_API_URL=$BACKEND_INTERNAL_URL" \
  -t "$FRONTEND_IMAGE" \
  "$(dirname "$0")/frontend"

log "Pushing frontend image"
docker push "$FRONTEND_IMAGE"

# ── 7. Deploy frontend ────────────────────────────────────────────────────────
log "Deploying frontend container app"

if az containerapp show --name "$FRONTEND_APP" --resource-group "$RESOURCE_GROUP" --output none 2>/dev/null; then
  # Update existing app — refresh image and backend URL env var
  az containerapp update \
    --name "$FRONTEND_APP" \
    --resource-group "$RESOURCE_GROUP" \
    --image "$FRONTEND_IMAGE" \
    --set-env-vars "NEXT_PUBLIC_API_URL=$BACKEND_INTERNAL_URL" \
    --output none
else
  # First-time create — set ingress, env vars
  az containerapp create \
    --name "$FRONTEND_APP" \
    --resource-group "$RESOURCE_GROUP" \
    --environment "$ENVIRONMENT" \
    --image "$FRONTEND_IMAGE" \
    --registry-server "$ACR_LOGIN_SERVER" \
    --registry-username "$ACR_USER" \
    --registry-password "$ACR_PASS" \
    --target-port 3000 \
    --ingress external \
    --min-replicas 1 \
    --max-replicas 3 \
    --env-vars \
        "NODE_ENV=production" \
        "NEXT_PUBLIC_API_URL=$BACKEND_INTERNAL_URL" \
    --output none
fi

FRONTEND_FQDN=$(az containerapp show \
  --name "$FRONTEND_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --query properties.configuration.ingress.fqdn -o tsv)

FRONTEND_URL="https://$FRONTEND_FQDN"
log "Frontend deployed → $FRONTEND_URL"

# ── 8. Update backend CORS_ORIGIN with real frontend URL ─────────────────────
log "Updating backend CORS_ORIGIN → $FRONTEND_URL"
az containerapp update \
  --name "$BACKEND_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --set-env-vars "CORS_ORIGIN=$FRONTEND_URL" \
  --output none

# ── Done ──────────────────────────────────────────────────────────────────────
echo
echo "════════════════════════════════════════════════════════"
echo "  EcoCode deployed successfully"
echo ""
echo "  Frontend :  $FRONTEND_URL"
echo "  Backend  :  $BACKEND_EXTERNAL_URL"
echo ""
echo "  CLI ingest endpoint:"
echo "    ECOCODE_ENDPOINT=$BACKEND_EXTERNAL_URL"
echo "════════════════════════════════════════════════════════"
