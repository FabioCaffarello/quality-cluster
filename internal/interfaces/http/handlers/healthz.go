package handlers

import (
	"net/http"
)

type HealthzWebHandler struct {
}

func NewHealthzWebHandler() *HealthzWebHandler {
	return &HealthzWebHandler{}
}

func (h *HealthzWebHandler) Healthz(w http.ResponseWriter, _ *http.Request) {
	writeStatusResponse(w, http.StatusOK, "ok")
}
