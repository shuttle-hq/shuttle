# Shuttle MCP Server

A Model Context Protocol (MCP) server that provides comprehensive access to Shuttle CLI commands for deployment and project management.

## Overview

This MCP server transforms all Shuttle CLI commands into MCP tools, enabling AI assistants to interact with Shuttle's deployment platform programmatically. It supports all major Shuttle operations including deployments, project management, resource management, SSL certificates, authentication, and utilities.

## Features

### Core Commands

- **Deployment**: Deploy projects with various options
- **Deployment Management**: List, status, redeploy, and stop deployments
- **Logging**: View build and deployment logs
- **Project Management**: Create, update, status, list, delete, and link projects
- **Resource Management**: List and delete project resources
- **SSL Certificate Management**: Add, list, and delete SSL certificates
- **Account Management**: Show account information
- **Utilities**: Generate shell completions, man pages, feedback, and upgrade CLI

## Installation

### Prerequisites

- Rust and Cargo installed
- Shuttle CLI installed and configured

### Set up nightly release channel

```bash
rustup toolchain install nightly
rustup default nightly
```

### Install MCP server the binary

```bash
cargo install --git https://github.com/dcodesdev/shuttle shuttle-mcp
```

## Configuration

### MCP Client Configuration

Add the following to your MCP client configuration:

```json
{
  "mcpServers": {
    "shuttle-tools": {
      "command": "shuttle-mcp"
    }
  }
}
```

That's it! 🚀 You can now use the **Shuttle MCP server** in your MCP client. ✨
