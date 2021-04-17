#!/bin/bash
# Generates the a CA cert, a server key, and a server cert signed by the CA.
# reference:
# https://github.com/kubernetes/kubernetes/blob/master/plugin/pkg/admission/webhook/gencerts.sh
set -o errexit
set -o nounset
set -o pipefail

CN_BASE="basic-validation-controller"
TMP_DIR="/tmp/webhook-certs"

echo "Generating certs for the Webhook Admission Controller in ${TMP_DIR}."
mkdir -p ${TMP_DIR}
cat > ${TMP_DIR}/server.conf << EOF
[req]
req_extensions = v3_req
distinguished_name = req_distinguished_name
[req_distinguished_name]
[ v3_req ]
basicConstraints = CA:FALSE
keyUsage = nonRepudiation, digitalSignature, keyEncipherment
extendedKeyUsage = clientAuth, serverAuth
subjectAltName = DNS:basic-validation-controller.default.svc
EOF

# Create a certificate authority
openssl genrsa -out ${TMP_DIR}/caKey.pem 2048
set +o errexit
openssl req -x509 -new -nodes -key ${TMP_DIR}/caKey.pem -days 100000 -out ${TMP_DIR}/caCert.pem -subj "/CN=${CN_BASE}_ca" -addext "subjectAltName = DNS:${CN_BASE}_ca"
if [[ $? -ne 0 ]]; then
  echo "ERROR: Failed to create CA certificate for self-signing. If the error is \"unknown option -addext\", update your openssl version."
  exit 1
fi
set -o errexit

# Create a server certiticate
openssl genrsa -out ${TMP_DIR}/serverKey.pem 2048
# Note the CN is the DNS name of the service of the webhook.
openssl req -new -key ${TMP_DIR}/serverKey.pem -out ${TMP_DIR}/server.csr -subj "/CN=basic-validation-controller.default.svc" -config ${TMP_DIR}/server.conf -addext "subjectAltName = DNS:basic-validation-controller.default.svc"
openssl x509 -req -in ${TMP_DIR}/server.csr -CA ${TMP_DIR}/caCert.pem -CAkey ${TMP_DIR}/caKey.pem -CAcreateserial -out ${TMP_DIR}/serverCert.pem -days 100000 -extensions SAN -extensions v3_req -extfile ${TMP_DIR}/server.conf

echo "Uploading certs to the cluster."
kubectl create secret --namespace=default generic webhook-tls-certs --from-file=${TMP_DIR}/caKey.pem --from-file=${TMP_DIR}/caCert.pem --from-file=${TMP_DIR}/serverKey.pem --from-file=${TMP_DIR}/serverCert.pem

# Clean up after we're done.
echo "Deleting ${TMP_DIR}."
# rm -rf ${TMP_DIR}