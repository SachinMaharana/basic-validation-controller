## Basic Validation Controller

This is a dead simple validating admission webhook controller for kubernetes that allows images of verfied container registry to be deployed in the cluster.
images of type `docker.io/nginx:1.19`, `gcr.io/nginx:1.19` are whitelisted but images of tag `nginx:1.19` are disallowed. The list of whitelisted registries is configurable through environment variable. The motivation for such controller aims to allow only trusted, secure images in the cluster. This can also be helpful to prevent docker rate-limting on the number of the images that could be pulled from docker registry.

## Prerequisites

- docker - 19.03.12+ (https://docs.docker.com/engine/install/ubuntu/)

* kind - v0.9.0+ (https://kind.sigs.k8s.io/docs/user/quick-start/#installation)

* kubectl (https://kubernetes.io/docs/tasks/tools/install-kubectl-linux/#install-kubectl-binary-with-curl-on-linux)

* rust - 1.50.0+ (https://rustup.rs/)

- just - v0.9.0 (https://github.com/casey/just#pre-built-binaries)

* jq - jq-1.5-1

## Complete guide to develop and deploy this admission controller

Assuming the prerequisites has been met/installed, let's continue with the workflow.

1. **Clone this project**

```bash
{
    git clone https://github.com/SachinMaharana/basic-validation-controller
    cd basic-validation-controller
}
```

2. **Update variables in Justfile**

A Justfile is provided in the repo to manage this project.

`docker_user` is the username of your dockerhub account. We will use it to tag our images.

`cluster_name` is the name of the cluster.

_Others can be left as default_

```bash
cluster_name := "gitter"
docker_user := "sachinnicky"
binary := "basic-validation-controller"
default_namespace := "default"
```

3. **Create a local dev kubernetes cluster**

```bash
just cluster-up
```

4. **Verify cluster is up**

```bash
$ kind get clusters
gitter

$ kubectl get nodes
NAME                   STATUS   ROLES    AGE     VERSION
gitter-control-plane   Ready    master   2m41s   v1.19.1
gitter-worker          Ready    <none>   2m9s    v1.19.1
gitter-worker2         Ready    <none>   2m9s    v1.19.1
gitter-worker3         Ready    <none>   2m15s   v1.19.1
```

5. **Generate tls certificates for HTTPS**

```bash
# give permission to execute
$ chmod +x ./gencert.sh

$ just certs

# verify
$ kubectl get secret webhook-tls-certs
NAME                TYPE     DATA   AGE
webhook-tls-certs   Opaque   4      34s
```

6. **Deploy the ValidationWebhookConfiguration**

```bash
$ just ca


$ kubectl get validationwebhookconfiguration.admissionregistration.k8s.io
NAME WEBHOOKS AGE
basic-validation-controller 1 27s

```

7. **Build and create docker image**

```bash
$ just build

# verify
$ docker images
REPOSITORY                                    TAG                 IMAGE ID            CREATED             SIZE
sachinnicky/basic-validation-controller   latest              38baba376694        1 hours ago         98.8MB
```

8. **Make it available for the cluster**

We can either push it to dockerhub and refer it in our deployment manifest or load the image into our cluster. We will go with second approach.

```bash
$ just load
```

9. **Deploy the controller**

```bash
$ just deploy

# verify
$ kubectl get pods
NAME                                               READY   STATUS    RESTARTS   AGE
basic-validation-controller-764bd94bdc-2kb62   1/1     Running   0          82s
```

10. **Deploy the debug pods to verify**

```bash
$ just debug

# verify
$ kubectl get po
```

11 . **Destroy the cluster**

```bash
just cluster-down
```
