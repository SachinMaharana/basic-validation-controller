cluster_name := "opa"
docker_user := "sachinnicky"
binary := "basic-validation-controller"
default_namespace := "default"

cluster-up:
    kind create cluster --name {{cluster_name}} --image kindest/node:v1.27.3  --config ./kind-config.yaml
    sleep "10"
    kubectl wait --namespace kube-system --for=condition=ready pod --selector="tier=control-plane" --timeout=180s

certs:
    ./gencert.sh --service basic-validation-controller --secret webhook-tls-certs --namespace {{default_namespace}}

ca default=default_namespace:
    #!/bin/bash
    CA_BUNDLE=$(kubectl get secrets -n {{default}} webhook-tls-certs -ojson | jq '.data."caCert.pem"')
    export CA_BUNDLE=${CA_BUNDLE}
    export NAMESPACE={{default}}
    cat deploy/webhook.yaml | envsubst > deploy/webhook-ca.yaml
    kubectl apply -f deploy/webhook-ca.yaml

build:
    cargo build --release && cp target/release/{{binary}} . && docker build -t {{docker_user}}/{{binary}} -f Dockerfile.alt . 
    
build-ci:
    docker build -t {{docker_user}}/{{binary}} .

build-go:
    #!/bin/bash
    pushd golang/ && go build -o basic-validation-controller && popd
    docker build -t {{docker_user}}/{{binary}} -f golang/Dockerfile golang/

load:
    kind --name {{cluster_name}} load docker-image {{docker_user}}/{{binary}}:latest

deploy: 
    #!/bin/bash
    export NAMESPACE={{default_namespace}}
    export IMAGE={{docker_user}}/{{binary}}
    cat deploy/deployment.yaml | envsubst | kubectl apply -f -
    kubectl rollout status --namesepace {{default_namespace}} deployment/{{binary}}

debug:
    #!/bin/bash
    export NAMESPACE={{default_namespace}}
    cat deploy/debug.yaml | envsubst | kubectl apply -f -

cluster-down:
    kind delete cluster --name {{cluster_name}}

all: cluster-up certs ca build load deploy

dl:
    kubectl delete -f deploy/deployment.yaml -f deploy/debug.yaml