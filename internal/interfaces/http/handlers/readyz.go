package handlers

import (
	"context"
	"net/http"
)

type ReadinessChecker interface {
	Check(context.Context) error
}

type ReadinessCheckerFunc func(context.Context) error

func (f ReadinessCheckerFunc) Check(ctx context.Context) error {
	return f(ctx)
}

func NewAlwaysReadyChecker() ReadinessChecker {
	return ReadinessCheckerFunc(func(context.Context) error { return nil })
}

type ReadyzWebHandler struct {
	checker ReadinessChecker
}

func NewReadyzWebHandler(checker ReadinessChecker) *ReadyzWebHandler {
	if checker == nil {
		checker = NewAlwaysReadyChecker()
	}

	return &ReadyzWebHandler{
		checker: checker,
	}
}

func (h *ReadyzWebHandler) Readyz(w http.ResponseWriter, r *http.Request) {
	if err := h.checker.Check(r.Context()); err != nil {
		writeStatusResponse(w, http.StatusServiceUnavailable, "not_ready")
		return
	}

	writeStatusResponse(w, http.StatusOK, "ready")
}
