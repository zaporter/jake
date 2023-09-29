build-docker:
	docker build -t llama . -f ./docker/Dockerfile.base

run-docker:
	docker run --gpus all --ipc=host --ulimit memlock=-1 --ulimit stack=67108864 -p 9090:9090 --rm -it --privileged -v "${PWD}":/app -w /app llama "/bin/bash"

run-nvidia-docker:
	nvidia-docker run --gpus all --ipc=host --ulimit memlock=-1 --ulimit stack=67108864 --rm -it --privileged -v "${PWD}":/app -w /app llama "/bin/bash"

