doc-server:
	docker run -u "$(id -u):$(id -g)" -v ${PWD}/doc-site:/app --workdir /app -p 8080:8080 -p 1024:1024 balthek/zola:0.13.0 serve --interface 0.0.0.0 --port 8080 --base-url localhost

build-docs:
	docker run -u "$(id -u):$(id -g)" -v ${PWD}/doc-site:/app --workdir /app balthek/zola:0.13.0 build

.PHONY: doc-server build-docs
