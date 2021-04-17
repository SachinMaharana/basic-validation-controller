## Image Tag Constraint Controller

---

## Prerequisites

- docker - 19.03.12+ (https://docs.docker.com/engine/install/ubuntu/)

* kind - v0.9.0+ (https://kind.sigs.k8s.io/docs/user/quick-start/#installation)

* kubectl (https://kubernetes.io/docs/tasks/tools/install-kubectl-linux/#install-kubectl-binary-with-curl-on-linux)

* rust - 1.50.0+ (https://rustup.rs/)

- just - v0.9.0 (https://github.com/casey/just#pre-built-binaries)

* jq

## Complete guide to develop and deploy this admission controller

---

Assuming the prerequisites has been met/installed, let's continue with the workflow.

1. **Clone this project**

```bash
{
    git clone https://github.com/SachinMaharana/image-tag-constraint-controller
    cd image-tag-constraint-controller
}
```

2. **Update variables in Justfile**

`docker_user` is the username of your dockerhub account. We will use it to tag our images.

`cluster_name` is the name of the cluster.

_Others can be left as default_

```bash
cluster_name := "gitter"
docker_user := "sachinnicky"
binary := "image-tag-constraint-controller"
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

$ ./gencert.sh --service image-tag-constraint-controller --secret webhook-tls-certs --namespace default

$ kubectl get secret webhook-tls-certs
NAME                TYPE     DATA   AGE
webhook-tls-certs   Opaque   4      34s
```

6. **Deploy the MutatingWebhookConfiguration**

```bash
# get the ca.cert token in base64 encoded. copy the token
$ just ca

$ export CA_BUNDLE=<paste token here>

$ cat deploy/webhook.yaml | envsubst > deploy/webhook-ca.yaml

# verify
$ cat deploy/webhook-ca.yaml

#install the MutatingWebhookConfiguration
$ kubectl apply -f deploy/webhook-ca.yaml

$ kubectl get mutatingwebhookconfiguration.admissionregistration.k8s.io
NAME                              WEBHOOKS   AGE
image-tag-constraint-controller   1          27s
```

7. **Build and create docker image**

```bash
$ just bld

# verify
$ docker images
REPOSITORY                                    TAG                 IMAGE ID            CREATED             SIZE
sachinnicky/image-tag-constraint-controller   latest              38baba376694        1 hours ago         98.8MB
```

8. **Make it available for the cluster**

We can either push it to dockerhub and refer it in our deployment manifest or load the image into our cluster. We will go with second approach.

```bash
$ just load
```

9. **Deploy the controller**

```bash
$ kubectl apply -f deploy/deployment.yaml

# verify
$ kubectl get pods
NAME                                               READY   STATUS    RESTARTS   AGE
image-tag-constraint-controller-764bd94bdc-2kb62   1/1     Running   0          82s

# If deployed succesfully we will se the following logs
$ kubectl logs  -l app=image-tag-constraint-controller  -f
Apr 14 18:20:21.259  INFO image_tag_constraint_controller: Started http server: 127.0.0.1:8443
```

10. **Deploy the debug pods to verify**

We see that the image in the manifest file of debug.yaml has image as `image: "nginx:>= 1.16, < 1.18"`. This is the constraint. This controller will resolve/find a image satisfying this constraint and deploy the pod.

```bash
$ kubectl apply -f deploy/debug.yaml

# verify
$ kubectl get po

# logs
$ kubectl logs  -l app=image-tag-constraint-controller  -f

Apr 14 18:20:21.259  INFO image_tag_constraint_controller: Started http server: 127.0.0.1:8443
Apr 14 18:23:46.628  INFO image_tag_constraint_controller: reqesut object: Kind="Pod", Namespace=default, OperationType=CREATE, Resource=debug-5fd78bff56-
Apr 14 18:23:46.629  INFO image_tag_constraint_controller: mutation to continue for image nginx:>= 1.16, < 1.18
Apr 14 18:23:48.286  INFO image_tag_constraint_controller: selected 1.17.10 image tag for image "nginx:>= 1.16, < 1.18"
Apr 14 18:23:48.296  INFO image_tag_constraint_controller: reqesut object: Kind="Pod", Namespace=default, OperationType=CREATE, Resource=another-858f6b657b-
Apr 14 18:23:48.297  INFO image_tag_constraint_controller: mutation to continue for image busybox:>= 1.32.0, < 1.32.1
Apr 14 18:23:49.071  INFO image_tag_constraint_controller: selected 1.32.0 image tag for image "busybox:>= 1.32.0, < 1.32.1"
```

11 . **Destroy the cluster**

```bash
just cluster-down
```

### Notes

webhook | allowed | HTTPOnly | errorAPP | Observe  
-------   -------   --------   --------   --------
Fail       false     Yes Ok      yes       Denied
Fail       false     Yes InSver  yes       Denied *
Fail       false     YesA Ok     yes       Denied  ** extra

Fail       true      Yes Ok      yes       Allowed -> make sense
Fail       true      Yes InSvEr  yes       Denied *  looks right
Fail       true      YesA Ok     yes       Allowed

//explicit denying. even if admissionwebhook goes down it wont rupture other stuff. is it that important of controller?

Ignore     false     YesA Ok      yes       Denied  ** // extra do this
Ignore     true      YesA Ok      yes       Allowed   right

Ignore     true      Yes Ok      yes       Allowed
Ignore     true      Yes InSvEr  yes       Allowed
Ignore     false     Yes Ok      yes       Denied    // makes sense
Ignore     false     Yes InSvEr  yes       Allowed // really ignoring

// Ignore

A wrapping ar in Ok
* Internal error occurred: failed calling webhook "image-tag-constraint-controller.default.svc.cluster.local": an error on the server ("unknown") has prevented the request from succeeding                                                                                

** Error creating: admission webhook "image-tag-constraint-controller.default.svc.cluster.local" denied the request without explanation


Hi. I using kube-rs to write a mutating admission controller. I am seeing certain issue which i intend to highligh here.

```rust
fn admission_error(req: AdmissionRequest<DynamicObject>, err: anyhow::Error, code: i32) -> HttpResponse {

    error!("error in admission: {}", err.to_string());
    let mut resp = AdmissionResponse::from(&req);
    resp.result = Status {
        code: Some(code),
        message: Some("Can i see this message?".to_string()),
        reason: Some(err.to_string()),
        ..Default::default()
    };
    resp.allowed = false;
    HttpResponse::Ok().json(resp.into_review())
}
```
Here i see the controller is working as expected with the following log.
ERROR image_tag_constraint_controller: error in admission: error with request namespace. cannot inject pod into system ns kube-system.

But i don't see the message/reason for this in the describe resource(replicaSet).

Warning  FailedCreate  2s (x12 over 12s)  replicaset-controller  Error creating: admission webhook "image-tag-constraint-controll │
│ er.default.svc.cluster.local" denied the request without explanation

Here i expected to see the message why this was denied.