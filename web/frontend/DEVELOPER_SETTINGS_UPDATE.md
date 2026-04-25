# Developer Settings Page - API Key Usage Guide

## What's New

The Developer Settings page now includes a comprehensive usage guide that teaches users the three ways to use their API keys with the EcoCode CLI.

## New Sections Added

### 1. **Usage Guide Section** (Main Feature)

A prominent section that explains all three methods of providing API keys:

#### Method 1: Save Once (Recommended) 🟢
- **Badge**: Green "RECOMMENDED" badge
- **Description**: Save API key once in `~/.ecocode/config.json`
- **Code Example**:
  ```bash
  # Save your API key once
  ecocode config set-api-key YOUR_API_KEY
  
  # Now use without --api-key flag
  ecocode --output online -- python train.py
  ```

#### Method 2: Environment Variable 🔵
- **Badge**: Blue "CI/CD" badge
- **Description**: Great for CI/CD pipelines, Docker, shell profiles
- **Code Example**:
  ```bash
  # Set for current session
  export ECOCODE_API_KEY=YOUR_API_KEY
  
  # Or add to ~/.bashrc or ~/.zshrc
  echo 'export ECOCODE_API_KEY=YOUR_KEY' >> ~/.bashrc
  
  # Use the CLI
  ecocode --output online -- python train.py
  ```

#### Method 3: Command-Line Flag 🟡
- **Badge**: Amber "OVERRIDE" badge
- **Description**: Provide key directly, overrides all other methods
- **Code Example**:
  ```bash
  ecocode --output online --api-key YOUR_API_KEY -- python train.py
  ```

### 2. **Priority Order Info Box**

A highlighted box explaining the priority order:
1. Command-line flag `--api-key` (highest)
2. Environment variable `ECOCODE_API_KEY`
3. Saved config file `~/.ecocode/config.json` (lowest)

### 3. **Additional Config Commands**

Two-column grid showing:
- `ecocode config show` - View current configuration
- `ecocode config clear-api-key` - Remove saved API key

### 4. **Enhanced New Key Alert**

When a new API key is generated, users now see:
- The API key (as before)
- **NEW**: Quick Start guide with the exact command to save the key
- Direct copy-paste command: `ecocode config set-api-key {actual_key}`
- Link to full usage guide below

## Visual Design

### Color Coding
- **Green/Emerald**: Recommended method (saved config)
- **Blue**: CI/CD method (environment variable)
- **Amber**: Override method (command-line flag)
- **Neutral**: Background and borders

### Layout
- Responsive design (mobile-friendly)
- Code blocks with syntax highlighting
- Clear visual hierarchy with badges
- Consistent spacing and padding

### Typography
- Headers: Bold, clear hierarchy
- Code: Monospace font with colored backgrounds
- Descriptions: Smaller, muted text for readability

## User Benefits

1. **Discoverability**: Users immediately see all options when they get their API key
2. **Education**: Clear explanations of when to use each method
3. **Copy-Paste Ready**: All commands are ready to copy and use
4. **Priority Clarity**: Users understand which method takes precedence
5. **Quick Start**: New key generation includes immediate next steps

## Technical Details

- **File**: `web/frontend/app/[workspaceSlug]/settings/developer/page.tsx`
- **Framework**: Next.js 16 with TypeScript
- **Styling**: Tailwind CSS
- **Icons**: Lucide React
- **Build Status**: ✅ Successful

## Screenshots Description

### Before
- Simple page with API key generation
- No usage instructions
- Users had to figure out how to use the key

### After
- Comprehensive usage guide above the API keys list
- Three clearly labeled methods with examples
- Priority order explanation
- Quick start guide when generating new keys
- Additional config commands reference

## Future Enhancements

Potential additions:
- Interactive tutorial/walkthrough
- Video guide link
- Copy button for each code example
- Test connection button
- Usage statistics (how many times the key was used)
