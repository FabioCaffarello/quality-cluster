package handlers

import (
	"encoding/json"
	"net/http"
)

const contentTypeJSON = "application/json"

type statusResponse struct {
	Status string `json:"status"`
}

func writeStatusResponse(w http.ResponseWriter, code int, status string) {
	w.Header().Set("Content-Type", contentTypeJSON)
	w.WriteHeader(code)

	_ = json.NewEncoder(w).Encode(statusResponse{Status: status})
}
