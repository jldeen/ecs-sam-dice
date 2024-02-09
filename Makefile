define get_output
	`aws cloudformation --region us-east-1 describe-stacks --stack-name sam-dice --query "Stacks[0].Outputs[?OutputKey=='$(1)'].OutputValue" --output text`
endef

REGISTRY := $(call get_output,RepositoryUrl)

oidc:
	aws cloudformation deploy \
	--stack-name oidc \
	--capabilities CAPABILITY_IAM \
	--template-file ./iac/oidc.yaml

all:
	@echo "REGISTRY=> $(REGISTRY)"

.PHONY: demo push upload-image upload-video

build:
	docker build ./fargate -t $(REGISTRY):latest

login:
	aws ecr get-login-password --region us-east-1 | docker login --username AWS --password-stdin ACCOUNTID.dkr.ecr.us-east-1.amazonaws.com

demo:
	cd iac && sam build && sam deploy

push:
	docker build ./fargate -t $(REGISTRY):latest
	docker push $(REGISTRY):latest

cleanup:
	cd iac && sam delete
