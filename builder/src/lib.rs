use askama::Template;
use shuttle_common::models::deployment::BuildArgsRust;

#[derive(Template)]
#[template(path = "rust.Dockerfile.jinja2", escape = "none")]
pub struct RustDockerfile<'a> {
    /// local or remote image name for the chef image
    pub chef_image: &'a str,
    /// content of inlined chef dockerfile
    pub cargo_chef_dockerfile: Option<&'a str>,
    /// local or remote image name for the runtime image
    pub runtime_image: &'a str,
    /// content of inlined runtime dockerfile
    pub runtime_base_dockerfile: Option<&'a str>,
    pub build_args: &'a BuildArgsRust,
}

pub fn render_rust_dockerfile(build_args: &BuildArgsRust) -> String {
    RustDockerfile {
        chef_image: "cargo-chef",
        cargo_chef_dockerfile: Some(include_str!("../templates/cargo-chef.Dockerfile")),
        runtime_image: "runtime-base",
        runtime_base_dockerfile: Some(include_str!("../templates/runtime-base.Dockerfile")),
        build_args,
    }
    .render()
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_str_eq;

    #[test]
    fn rust_basic() {
        let t = RustDockerfile {
            chef_image: "chef",
            cargo_chef_dockerfile: Some("foo"),
            runtime_image: "rt",
            runtime_base_dockerfile: Some("bar"),
            build_args: &BuildArgsRust {
                package_name: Some("hello".into()),
                features: Some("asdf".into()),
                ..Default::default()
            },
        };

        let s = t.render().unwrap();

        assert!(s.contains("foo\n\n"));
        assert!(s.contains("bar\n\n"));
        assert!(s.contains("FROM chef AS chef"));
        assert!(s.contains("FROM rt AS runtime"));
        assert!(s.contains("RUN cargo chef cook --release --package hello --features asdf\n"));
        assert!(s.contains("mv /app/target/release/hello"));
    }

    #[test]
    fn rust_full() {
        let s = render_rust_dockerfile(&BuildArgsRust {
            package_name: Some("hello".into()),
            features: Some("asdf".into()),
            ..Default::default()
        });
        assert_str_eq!(s, include_str!("../tests/rust.Dockerfile"));
    }
}
