PROJECT_ID ?= zero2prod-1790050760
REGION ?= asia-southeast1
REPOSITORY ?= zero2prod-repo
SERVICE_NAME ?= zero2prod
IMAGE_TAG ?= alpine
IMAGE_URI = $(REGION)-docker.pkg.dev/$(PROJECT_ID)/$(REPOSITORY)/$(SERVICE_NAME):$(IMAGE_TAG)

PROJECT_NUMBER = $(shell gcloud projects describe $(PROJECT_ID) --format="value(projectNumber)")
SERVICE_ACCOUNT = $(PROJECT_NUMBER)-compute@developer.gserviceaccount.com

.PHONY: help build push deploy release logs url add-secret update-secret remove-secret list-secrets test

help: ## Show this help message
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-18s\033[0m %s\n", $$1, $$2}'

# -----------------------------------------------------------------------------
# Deployment Commands
# -----------------------------------------------------------------------------

build: ## Build the multi-stage Alpine Docker image
	docker build --tag $(IMAGE_URI) --file Dockerfile .

push: ## Push Docker image to Google Artifact Registry
	docker push $(IMAGE_URI)

deploy: ## Apply service.yaml spec to Cloud Run
	gcloud run services replace service.yaml --region $(REGION)

release: build push deploy ## Full release pipeline: build -> push -> deploy

logs: ## Tail live logs from Cloud Run service
	gcloud run services logs tail $(SERVICE_NAME) --region $(REGION)

url: ## Get the public service URL and run a health check
	@echo "Service URL: $$(gcloud run services describe $(SERVICE_NAME) --region $(REGION) --format='value(status.url)')"
	@curl -i "$$(gcloud run services describe $(SERVICE_NAME) --region $(REGION) --format='value(status.url)')/health_check"

# -----------------------------------------------------------------------------
# Secret Management Commands
# -----------------------------------------------------------------------------

add-secret: ## Add a secret (reads from .env if omitted) and grant Cloud Run access. Usage: make add-secret NAME=<name> [VALUE=<val>]
	@if [ -z "$(NAME)" ]; then echo "Error: NAME is required. Usage: make add-secret NAME=<secret-name> [VALUE=<secret-value>]"; exit 1; fi
	@val="$(VALUE)"; \
	if [ -z "$$val" ] && [ -f .env ]; then \
		env_key=$$(echo "$(NAME)" | tr '[:lower:]-' '[:upper:]_'); \
		val=$$(grep -E "^($(NAME)|$$env_key)=" .env 2>/dev/null | head -n 1 | sed -E 's/^[^=]+=(.*)$$/\1/' | sed -e 's/^"//' -e 's/"$$//' -e "s/^'//" -e "s/'$$//"); \
		if [ -n "$$val" ]; then echo "Loaded value for $(NAME) from .env"; fi; \
	fi; \
	if [ -z "$$val" ]; then \
		read -sp "Enter secret value for $(NAME): " val; echo ""; \
	fi; \
	if [ -z "$$val" ]; then echo "Error: secret value is empty."; exit 1; fi; \
	printf "%s" "$$val" | gcloud secrets create $(NAME) --data-file=- --replication-policy="automatic"; \
	echo "Granting secretAccessor role to Cloud Run service account ($(SERVICE_ACCOUNT))..."; \
	gcloud secrets add-iam-policy-binding $(NAME) \
		--member="serviceAccount:$(SERVICE_ACCOUNT)" \
		--role="roles/secretmanager.secretAccessor"; \
	echo "Secret $(NAME) created and configured successfully."

update-secret: ## Add a new version to an existing secret (reads from .env if omitted). Usage: make update-secret NAME=<name> [VALUE=<val>]
	@if [ -z "$(NAME)" ]; then echo "Error: NAME is required. Usage: make update-secret NAME=<secret-name> [VALUE=<secret-value>]"; exit 1; fi
	@val="$(VALUE)"; \
	if [ -z "$$val" ] && [ -f .env ]; then \
		env_key=$$(echo "$(NAME)" | tr '[:lower:]-' '[:upper:]_'); \
		val=$$(grep -E "^($(NAME)|$$env_key)=" .env 2>/dev/null | head -n 1 | sed -E 's/^[^=]+=(.*)$$/\1/' | sed -e 's/^"//' -e 's/"$$//' -e "s/^'//" -e "s/'$$//"); \
		if [ -n "$$val" ]; then echo "Loaded value for $(NAME) from .env"; fi; \
	fi; \
	if [ -z "$$val" ]; then \
		read -sp "Enter new secret value for $(NAME): " val; echo ""; \
	fi; \
	if [ -z "$$val" ]; then echo "Error: secret value is empty."; exit 1; fi; \
	printf "%s" "$$val" | gcloud secrets versions add $(NAME) --data-file=-; \
	echo "New version added to $(NAME)."


remove-secret: ## Delete a secret. Usage: make remove-secret NAME=<name>
	@if [ -z "$(NAME)" ]; then echo "Error: NAME is required. Usage: make remove-secret NAME=<secret-name>"; exit 1; fi
	gcloud secrets delete $(NAME) --quiet
	@echo "Secret $(NAME) deleted."

list-secrets: ## List all secrets in Secret Manager
	gcloud secrets list

# -----------------------------------------------------------------------------
# Local Development
# -----------------------------------------------------------------------------

test: ## Run integration tests with local postgres
	DATABASE_URL=postgres://postgres:password@localhost:5432/newsletter cargo test
