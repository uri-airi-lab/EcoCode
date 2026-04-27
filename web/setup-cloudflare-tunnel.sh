#!/bin/bash

# EcoCode Cloudflare Tunnel Setup
# This script sets up a secure tunnel to expose your app to the internet
# No port forwarding needed!

set -e

echo "🌐 EcoCode Cloudflare Tunnel Setup"
echo "=================================="
echo ""

# Check if cloudflared is installed
if ! command -v cloudflared &> /dev/null; then
    echo "📦 Installing Cloudflare Tunnel (cloudflared)..."
    
    # Detect OS and install accordingly
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        # Linux
        wget -q https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64.deb
        sudo dpkg -i cloudflared-linux-amd64.deb
        rm cloudflared-linux-amd64.deb
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        brew install cloudflared
    else
        echo "❌ Unsupported OS. Please install cloudflared manually."
        exit 1
    fi
    
    echo "✅ Cloudflared installed successfully!"
fi

echo ""
echo "🔐 Setting up Cloudflare authentication..."
echo "This will open a browser window for you to log in to Cloudflare."
echo "Press Enter to continue..."
read

# Authenticate with Cloudflare
cloudflared tunnel login

echo ""
echo "📝 Enter your domain name:"
echo "Options:"
echo "  1. Free domain from freenom.com (yourname.tk)"
echo "  2. DuckDNS domain (yourname.duckdns.org)"
echo "  3. Your own domain"
echo ""
read -p "Domain name: " DOMAIN

if [[ -z "$DOMAIN" ]]; then
    echo "❌ Domain name is required!"
    exit 1
fi

# Create tunnel
TUNNEL_NAME="ecocode-tunnel"
echo ""
echo "🚇 Creating Cloudflare tunnel: $TUNNEL_NAME"
cloudflared tunnel create $TUNNEL_NAME

# Get tunnel ID
TUNNEL_ID=$(cloudflared tunnel list | grep $TUNNEL_NAME | awk '{print $1}')

if [[ -z "$TUNNEL_ID" ]]; then
    echo "❌ Failed to create tunnel!"
    exit 1
fi

echo "✅ Tunnel created with ID: $TUNNEL_ID"

# Create tunnel configuration
echo ""
echo "📝 Creating tunnel configuration..."

mkdir -p ~/.cloudflared

cat > ~/.cloudflared/config.yml << EOF
tunnel: $TUNNEL_ID
credentials-file: ~/.cloudflared/$TUNNEL_ID.json

ingress:
  # API routes go to backend
  - hostname: $DOMAIN
    path: /api/*
    service: http://localhost:3001
  
  # Everything else goes to frontend
  - hostname: $DOMAIN
    service: http://localhost:3000
  
  # Catch-all rule (required)
  - service: http_status:404
EOF

echo "✅ Configuration created at ~/.cloudflared/config.yml"

# Create DNS record
echo ""
echo "🌐 Creating DNS record..."
cloudflared tunnel route dns $TUNNEL_NAME $DOMAIN

echo ""
echo "🔄 Updating application configuration for domain access..."

# Update docker-compose.yml to use domain
cat > docker-compose.yml << EOF
# EcoCode — Docker Compose
# Backend uses Neon (cloud Postgres), so no local db service needed.
# Env vars for backend are loaded from ./backend/.env

services:

  backend:
    build:
      context: ./backend
      dockerfile: Dockerfile
    container_name: ecocode-backend
    restart: unless-stopped
    env_file:
      - ./backend/.env          # loads DATABASE_URL, JWT_SECRET, CORS_ORIGIN, PRODUCTION
    ports:
      - "127.0.0.1:3001:3001"   # only localhost access (tunnel will proxy)

  frontend:
    build:
      context: ./frontend
      dockerfile: Dockerfile
      args:
        # Use domain for API access
        NEXT_PUBLIC_API_URL: https://$DOMAIN/api
    container_name: ecocode-frontend
    restart: unless-stopped
    environment:
      NODE_ENV: production
      NEXT_PUBLIC_API_URL: https://$DOMAIN/api
    ports:
      - "127.0.0.1:3000:3000"   # only localhost access (tunnel will proxy)
    depends_on:
      - backend

EOF

# Update backend .env
echo ""
echo "🔧 Updating backend configuration..."

# Backup current .env
cp backend/.env backend/.env.backup.$(date +%Y%m%d_%H%M%S)

# Update CORS_ORIGIN and PRODUCTION settings
sed -i "s|CORS_ORIGIN=.*|CORS_ORIGIN=https://$DOMAIN|g" backend/.env
sed -i "s|PRODUCTION=.*|PRODUCTION=true|g" backend/.env

echo "✅ Backend configuration updated"

# Create systemd service for tunnel
echo ""
echo "🔧 Setting up tunnel as a system service..."

sudo tee /etc/systemd/system/cloudflared-tunnel.service > /dev/null << EOF
[Unit]
Description=Cloudflare Tunnel
After=network.target

[Service]
Type=simple
User=$USER
ExecStart=/usr/local/bin/cloudflared tunnel run
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
EOF

# Enable and start the service
sudo systemctl daemon-reload
sudo systemctl enable cloudflared-tunnel
sudo systemctl start cloudflared-tunnel

echo ""
echo "🎉 Setup Complete!"
echo "=================="
echo ""
echo "Your EcoCode app is now accessible at: https://$DOMAIN"
echo ""
echo "Next steps:"
echo "1. Rebuild and restart your containers:"
echo "   cd ~/Desktop/ecocode/web"
echo "   docker-compose down"
echo "   docker-compose up --build -d"
echo ""
echo "2. Check tunnel status:"
echo "   sudo systemctl status cloudflared-tunnel"
echo ""
echo "3. View tunnel logs:"
echo "   sudo journalctl -u cloudflared-tunnel -f"
echo ""
echo "🔒 Security Notes:"
echo "- Your app is now secured with HTTPS"
echo "- No ports need to be opened on your router"
echo "- Cloudflare provides DDoS protection"
echo ""
echo "📱 The app should now work perfectly on cellular data!"