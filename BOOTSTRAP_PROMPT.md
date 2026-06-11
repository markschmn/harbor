You are an expert Rust systems engineer, security engineer, UX designer, and software architect.

Your task is to autonomously design and implement a modern cross-platform SSH client and file transfer application in Rust.

# Project Goal

Build a modern alternative to PuTTY + WinSCP with an integrated GUI.

The application should provide:

- Secure SSH terminal access
- Secure file transfers
- Saved server profiles
- Modern user experience
- Native performance
- Cross-platform support (Windows first, Linux/macOS compatible)

Use professional software engineering practices and make reasonable architectural decisions without asking for approval unless absolutely necessary.

# Technology Requirements

Language:
- Rust stable

GUI:
- Tauri v2
- React + TypeScript frontend
- Modern responsive UI
- Dark/light mode support

Backend:
- Tokio async runtime

State Management:
- Use a clean architecture
- Separate UI, application logic, infrastructure, and domain layers

Configuration:
- TOML configuration files
- Serde serialization

Storage:
- Secure local storage
- OS keychain integration when possible

Version Control:
- Initialize a Git repository
- Create meaningful commits throughout development
- Use feature branches when appropriate
- Generate a professional .gitignore

# Security Requirements

Security is a primary goal.

DO NOT implement cryptographic primitives yourself.

Use proven libraries.

Requirements:

- Verify SSH host keys
- Maintain known_hosts compatibility
- Support SSH agent authentication
- Support password authentication
- Support private key authentication
- Support encrypted private keys
- Never disable host verification by default
- Never store passwords in plaintext
- Prefer OS keychain storage
- Follow OpenSSH security conventions whenever practical

# SSH and File Transfer Decision

Choose the most secure and maintainable architecture.

You may use:

- OpenSSH integration
- openssh-rs
- russh
- ssh2-rs

Evaluate tradeoffs and select the best solution.

For file transfer:

- Prefer SFTP over SCP unless there is a compelling technical reason not to.
- Explain the decision in architecture documentation.
- Implement upload and download support.

# Core Features

## Connection Manager

Allow users to save profiles.

Profile fields:

- Name
- Host
- Port
- Username
- Authentication type
- Key path
- Notes
- Tags

Features:

- Create profile
- Edit profile
- Delete profile
- Search profiles
- Favorite profiles

## SSH Terminal

Provide a fully interactive terminal.

Requirements:

- PTY support
- Resize support
- Multiple tabs
- Copy/paste
- Session reconnect handling
- UTF-8 support

Terminal experience should feel similar to modern terminals.

## File Browser

Dual-pane layout:

Left:
- Local filesystem

Right:
- Remote filesystem

Features:

- Upload
- Download
- Rename
- Delete
- Create directory
- Drag and drop
- Progress indicators
- Transfer queue

## Transfer Manager

Provide:

- Parallel transfers
- Transfer history
- Retry support
- Pause/resume when feasible
- Error reporting

## Key Management

Support:

- SSH keys
- SSH agent
- Existing OpenSSH keys

Provide:

- Key discovery
- Fingerprint display
- Validation

Do not generate custom key formats.

# Modern UX Requirements

The application should look like software released in 2026.

Avoid looking like:

- PuTTY
- WinSCP
- Legacy enterprise software

Desired qualities:

- Clean
- Minimal
- Fast
- Professional
- Modern spacing
- Smooth animations
- Keyboard shortcuts

Suggested layout:

Sidebar:
- Connections
- Transfers
- Keys
- Settings

Main area:
- Terminal tabs
- File browser
- Connection details

Create a polished design system.

# Architecture

Design the project with:

Backend:

- src/domain
- src/application
- src/infrastructure
- src/presentation

Frontend:

- components
- pages
- hooks
- services
- stores

Document all architecture decisions.

# Documentation

Generate:

- README.md
- ARCHITECTURE.md
- SECURITY.md
- ROADMAP.md

Explain:

- SSH library choice
- SFTP vs SCP decision
- Security model
- Threat considerations
- Future extensions

# Testing

Create:

- Unit tests
- Integration tests
- Security-focused tests

Test:

- Authentication
- Host verification
- File transfers
- Configuration loading

# Development Process

Work autonomously.

When uncertain:

1. Analyze alternatives.
2. Choose the most maintainable and secure option.
3. Document the reasoning.
4. Continue implementation.

Do not stop after planning.

Actually implement the project.

Create commits as meaningful milestones.

Prefer production-quality code over prototypes.

Avoid mock implementations unless absolutely necessary.

# Deliverables

At completion provide:

- Fully compilable Rust project
- Tauri GUI
- SSH functionality
- SFTP functionality
- Saved profiles
- Transfer manager
- Documentation
- Tests
- Git history

The final result should be a realistic open-source project that could serve as a modern replacement for PuTTY + WinSCP for professional developers and system administrators.
