extern crate regex;

use std::collections::HashMap;

use bollard::container::{
    AttachContainerOptions, Config, CreateContainerOptions, ListContainersOptions, LogOutput,
    LogsOptions, RemoveContainerOptions, StartContainerOptions,
};
use bollard::Docker;
use regex::Regex;

use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::image::CreateImageOptions;

use futures_util::stream::StreamExt;
use futures_util::TryStreamExt;
pub fn extract_commands(input: &str) -> Vec<String> {
    let pattern = r"(\[<)(?P<command>[^(>\])]*)(>\])";
    let re = Regex::new(pattern).unwrap();
    let mut commands = Vec::new();

    for cap in re.captures_iter(input) {
        if let Some(matched) = cap.name("command") {
            commands.push(matched.as_str().to_string());
        }
    }

    commands
}

pub struct NexosInstance {}
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct DockerResult {
    pub output: Vec<LogLine>,
    pub exit_code: i32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum LogLine {
    StdOut { message: String },
    StdErr { message: String },
}
impl NexosInstance {
    // This does not have to be mutable but I am using the borrow checker to ensure
    // this isn't concurrently modified
    pub async fn exec_simple(&mut self, command: &str) -> anyhow::Result<DockerResult> {
        // Create a Docker connection
        // Iterate over logs and print them based on their StreamType
        //
        let docker = Docker::connect_with_socket_defaults().unwrap();
        let IMAGE = "nexos:latest";

        // docker
        //     .create_image(
        //         Some(CreateImageOptions {
        //             from_image: IMAGE,
        //             ..Default::default()
        //         }),
        //         None,
        //         None,
        //     )
        //     .try_collect::<Vec<_>>()
        //     .await?;

        let alpine_config = Config {
            image: Some(IMAGE),
            tty: Some(true),
            host_config: Some(bollard::service::HostConfig {
                binds: Some(vec![
                    "/home/zack/personal/jake/nexos/persist:/home/jake".to_string()
                ]),
                ..Default::default()
            }),
            ..Default::default()
        };

        let id = docker
            .create_container::<&str, &str>(None, alpine_config)
            .await?
            .id;
        docker.start_container::<String>(&id, None).await?;

        // non interactive
        let exec = docker
            .create_exec(
                &id,
                CreateExecOptions {
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    cmd: Some(vec!["zsh", "-c", command]),
                    ..Default::default()
                },
            )
            .await?
            .id;
        let mut result = DockerResult::default();
        if let StartExecResults::Attached { mut output, .. } =
            docker.start_exec(&exec, None).await?
        {
            while let Some(Ok(msg)) = output.next().await {
                match msg {
                    LogOutput::StdOut { message } => {
                        let message = String::from_utf8_lossy(&message);
                        result.output.push(LogLine::StdOut {
                            message: message.into(),
                        });
                    }
                    LogOutput::StdErr { message } => {
                        let message = String::from_utf8_lossy(&message);
                        result.output.push(LogLine::StdErr {
                            message: message.into(),
                        });
                    }
                    _ => {}
                }
            }
        } else {
            unreachable!();
        }

        docker
            .remove_container(
                &id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await?;

        Ok(result)
    }
    pub async fn exec(&mut self, mut options: CreateExecOptions<String>) -> anyhow::Result<()> {
        options.attach_stdout = Some(true);
        options.attach_stderr = Some(true);
        let docker = Docker::connect_with_socket_defaults()?;

        // let container_config = bollard::container::Config {
        //     image: Some("ubuntu:23.10".into()),
        //     ..Default::default()
        // };

        // docker
        //     .create_image(
        //         Some(CreateImageOptions {
        //             from_image: "nexos".to_string(),
        //             ..Default::default()
        //         }),
        //         None,
        //         None,
        //     )
        //     .try_collect::<Vec<_>>()
        //     .await?;

        // let loptions: Option<ListContainersOptions<String>> = Some(ListContainersOptions {
        //     all: true,
        //     ..Default::default()
        // });

        // let cts = docker.list_containers(loptions).await?;
        // println!("{:?}", cts);
        let id = "4c826213ae04".to_string();

        // let id = docker
        //     .create_container::<&str, String>(None, container_config)
        //     .await?
        //     .id;

        docker.start_container::<String>(&id, None).await?;
        let exec = docker.create_exec(&id, options).await?.id;
        if let StartExecResults::Attached { mut output, .. } =
            docker.start_exec(&exec, None).await?
        {
            while let Some(Ok(msg)) = output.next().await {
                print!("{}", msg);
            }
        } else {
            unreachable!();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_commands() {
        let input_str = "Some random text [<sh command1>] and [<command2>] etc.";
        let commands = extract_commands(input_str);

        assert_eq!(commands, vec!["sh command1", "command2"]);

        // Additional test cases and assertions can be added here.
    }
}
