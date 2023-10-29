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
use strum_macros::Display;
// pub fn extract_commands(input: &str) -> Vec<String> {
//     let pattern = r"(\[<)(?P<command>[^(>\])]*)(>\])";
//     // let pattern = r"(\[<)(?P<command>.*?)(?=>\])(>\])";
//     let re = Regex::new(pattern).unwrap();
//     let mut commands = Vec::new();

//     for cap in re.captures_iter(input) {
//         if let Some(matched) = cap.name("command") {
//             commands.push(matched.as_str().to_string());
//         }
//     }

//     commands
// }
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Nexos(String),
    System(String),
}
pub fn extract_commands(s: &str) -> Vec<Command> {
    let mut results = Vec::new();
    let mut capture = false;
    let mut capture_char = '0';
    let mut buffer = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if i < chars.len() - 2 && chars[i] == '[' && (chars[i + 1] == '<' || chars[i + 1] == '(') {
            capture = true;
            match chars[i + 1] {
                '<' => capture_char = '>',
                '(' => capture_char = ')',
                _ => unreachable!(),
            }
            i += 1; // Skip the next character
        } else if i < chars.len() - 1 && capture && chars[i] == capture_char && chars[i + 1] == ']'
        {
            capture = false;

            let cmd = match capture_char {
                '>' => Command::Nexos(buffer.clone()),
                ')' => Command::System(buffer.clone()),
                _ => unreachable!(),
            };
            results.push(cmd);
            buffer.clear();
        } else if capture {
            buffer.push(chars[i]);
        }
        i += 1;
    }
    results
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
    pub async fn rebuild(&mut self) -> anyhow::Result<DockerResult> {
        let IMAGE = "nexos:latest";
        let docker = Docker::connect_with_socket_defaults().unwrap();
        let mut result = DockerResult::default();
        let mut tar = tar::Builder::new(Vec::new());
        tar.append_dir_all(".", "/home/zack/personal/jake/nexos/persist/System")?; // Tar up the current directory, which contains the Dockerfile

        let tarball = tar.into_inner()?;
        let mut image_build_stream = docker.build_image(
            bollard::image::BuildImageOptions {
                dockerfile: "Dockerfile.txt",
                t: IMAGE,
                rm: true,
                ..Default::default()
            },
            None,
            Some(tarball.into()),
        );
        while let Some(msg) = image_build_stream.next().await {
            println!("Message: {msg:?}");
            result.output.push(LogLine::StdErr {
                message: format!("{msg:?}"),
            })
        }
        return Ok(result);
    }
    // This does not have to be mutable but I am using the borrow checker to ensure
    // this isn't concurrently modified
    pub async fn exec_simple(&mut self, command: &str) -> anyhow::Result<DockerResult> {
        // Create a Docker connection
        // Iterate over logs and print them based on their StreamType
        //
        //
        let mut result = DockerResult::default();
        let docker = Docker::connect_with_socket_defaults().unwrap();
        let IMAGE = "nexos:latest";

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
                    cmd: Some(vec!["zsh", "-c", &format!("source ~/.zshrc; {}", command)]),
                    ..Default::default()
                },
            )
            .await?
            .id;
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
