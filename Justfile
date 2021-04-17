cluster_name := "opa"
docker_user := "sachinnicky"
binary := "basic-validation-controller"
default_namespace := "default"

cluster-up:
    kind create cluster --name {{cluster_name}} --image kindest/node:v1.19.1  --config ./kind-config.yaml
    sleep "15"

certs:
    ./gencert.sh
ca default=default_namespace:
    #!/bin/bash
    # echo 'kubectl get  MutatingWebhookConfiguration image-tag-constraint-controller  -ojson | jq '.webhooks[].clientConfig.caBundle="caBundleHere"' | kubectl apply -f -'
    CA_BUNDLE=$(kubectl get secrets -n {{default}} webhook-tls-certs -ojson | jq '.data."caCert.pem"')
    export CA_BUNDLE=${CA_BUNDLE}
    cat deploy/webhook.yaml | envsubst > deploy/webhook-ca.yaml
    kubectl apply -f deploy/webhook-ca.yaml
    
build:
    docker build -t {{docker_user}}/{{binary}} .

bld:
    cargo build --release && cp target/release/{{binary}} . && docker build -t {{docker_user}}/{{binary}} -f Dockerfile.alt . 

load:
    kind --name {{cluster_name}} load docker-image {{docker_user}}/{{binary}}:latest

deploy-up:
    kubectl apply -f deploy/deployment.yaml
    kubectl rollout status deployment/{{binary}}

debug:
    kubectl apply -f deploy/debug.yaml

cluster-down:
    kind delete cluster --name {{cluster_name}}

all: cluster-up certs ca bld load deploy

dl:
    kubectl delete -f deploy/deployment.yaml -f deploy/debug.yaml