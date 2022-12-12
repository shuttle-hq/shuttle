# shuttle-gateway

## Tests
To run the tests for gateway, follow the steps in [contributing](https://github.com/shuttle-hq/shuttle/blob/main/CONTRIBUTING.md) to set up your local environment. Then, from the root of the repository, run:

```bash
SHUTTLE_TESTS_RUNTIME_IMAGE=public.ecr.aws/shuttle-dev/deployer:latest SHUTTLE_TESTS_NETWORK=shuttle-dev_user-net cargo test --package shuttle-gateway --all-features -- --nocapture
```
