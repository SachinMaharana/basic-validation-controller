package main

import (
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"os"
	"strings"

	admissionv1 "k8s.io/api/admission/v1"
	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
)

func handleHealth(w http.ResponseWriter, r *http.Request) {
	w.WriteHeader(http.StatusOK)
	w.Header().Set("Content-Type", "application/json")
	w.Write([]byte(`{"status": "ok"}`))
}

func handleValidate(w http.ResponseWriter, r *http.Request) {
	log.Printf("Mutate called!")

	input := &admissionv1.AdmissionReview{}
	err := json.NewDecoder(r.Body).Decode(input)
	if err != nil {
		sendErr(w, fmt.Errorf("could not unmarshal review: %v", err))
		return
	}

	pod := &corev1.Pod{}

	err = json.Unmarshal(input.Request.Object.Raw, pod)
	if err != nil {
		sendErr(w, fmt.Errorf("could not unmarshal pod: %v", err))
		return
	}

	allowed := true
	message := ""

	whitelisted_registries := strings.Split(os.Getenv("WHITELISTED_REGISTRIES"), ",")

	log.Println("whitelisted_registries:", whitelisted_registries)

	containers := pod.Spec.Containers
	for _, container := range containers {
		whitelisted := false
		imageName := container.Image
	out:
		for _, reg := range whitelisted_registries {
			pattern := fmt.Sprintf("%s/", string(reg))
			log.Println("pattern >", pattern, "imageName >", imageName, "reg >", reg)
			if strings.HasPrefix(imageName, pattern) {
				whitelisted = true
				break out
			}
		}
		if !whitelisted {
			log.Println("Not whitelisted")
			allowed = false
			message = fmt.Sprintf("image %s comes from an untrusted registry! Only images from %v registries are allowed", imageName, whitelisted_registries)
		}
	}

	respReview := &admissionv1.AdmissionReview{
		TypeMeta: input.TypeMeta,
		Response: &admissionv1.AdmissionResponse{
			UID:     input.Request.UID,
			Allowed: allowed,
			Result: &metav1.Status{
				Message: message,
			},
		},
	}
	respBytes, err := json.Marshal(respReview)
	if err != nil {
		sendErr(w, fmt.Errorf("could not generate response: %v", err))
		return
	}

	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusOK)
	w.Write(respBytes)
}

func main() {
	mux := http.NewServeMux()
	mux.HandleFunc("/mutate", handleValidate)
	mux.HandleFunc("/healthz", handleHealth)
	log.Println("Starting Server at 8443")
	srv := &http.Server{Addr: ":8443", Handler: mux}
	log.Fatal(srv.ListenAndServeTLS("./certs/serverCert.pem", "./certs/serverKey.pem"))
}

func sendErr(w http.ResponseWriter, err error) {
	out, err := json.Marshal(map[string]string{"Err": err.Error()})
	if err != nil {
		http.Error(w, fmt.Sprintf("%v", err), http.StatusInternalServerError)
		return
	}

	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusInternalServerError)
	w.Write(out)
}
