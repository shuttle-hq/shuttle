# Shuttle MCP server

We need to build a MCP server for the Shuttle CLI. We transform all of the CLI commands into MCP tools.

Before each task use the `shuttle -h` command to double check the commands and options, and then use `shuttle <command> -h` to understand the options, args, etc.

Every command will have a required `cwd` argument, which will be passed to the `std::process::Command` as the current working directory.

## Core Commands

### Deployment

- [x] `shuttle deploy` - Deploy project

### Deployment Management

- [x] `shuttle deployment list` - List deployments
- [x] `shuttle deployment status` - View deployment status
- [x] `shuttle deployment redeploy` - Redeploy previous deployment
- [x] `shuttle deployment stop` - Stop running deployments

### Logging

- [x] `shuttle logs` - View build and deployment logs
  - Args: `[ID]` (optional deployment ID)
  - Options: `--latest`, `--raw`

### Project Management

- [x] `shuttle project create` - Create project on Shuttle
- [x] `shuttle project update` - Update project config
- [x] `shuttle project status` - Get project status
- [x] `shuttle project list` - List all accessible projects
- [x] `shuttle project delete` - Delete project and data
- [x] `shuttle project link` - Link workspace to Shuttle project

### Resource Management

- [ ] `shuttle resource list` - List project resources
- [ ] `shuttle resource delete` - Delete resource

### SSL Certificate Management

- [ ] `shuttle certificate add` - Add SSL certificate for custom domain
- [ ] `shuttle certificate list` - List project certificates
- [ ] `shuttle certificate delete` - Delete SSL certificate

### Authentication

- [ ] `shuttle login` - Log in to Shuttle platform
  - Options: `--prompt`, `--api-key`
- [ ] `shuttle logout` - Log out of Shuttle platform
  - Options: `--reset-api-key`

### Account Management

- [ ] `shuttle account` - Show account info

### Utilities

- [ ] `shuttle generate shell` - Generate shell completions
- [ ] `shuttle generate manpage` - Generate man page
- [ ] `shuttle feedback` - Open GitHub issue for feedback
- [ ] `shuttle upgrade` - Upgrade CLI binary
  - Options: `--preview`

## Global Options (apply to all commands)

- `--offline` - Disable non-essential network requests
- `--debug` - Turn on tracing output
- `--working-directory` / `--wd` - Specify working directory
- `--name` / `--id` - Specify project name or ID

## Implementation Notes

- All commands support global options
- Many commands have aliases (e.g., `depl` for `deployment`, `proj` for `project`)
- Login options are available for commands that might need authentication
- Raw output options for machine-readable formats
