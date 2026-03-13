package handlers

import (
	"net/http"
)

type ConfigctlWebWandler struct {
}

func NewConfigctlWebWandler() *ConfigctlWebWandler {
	return &ConfigctlWebWandler{}
}

func (h *ConfigctlWebWandler) CreateConfig(w http.ResponseWriter, _ *http.Request) {
	writeStatusResponse(w, http.StatusOK, "ok")
}
