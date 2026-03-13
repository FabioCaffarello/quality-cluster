package handlers

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestReadyz(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name       string
		checker    ReadinessChecker
		wantCode   int
		wantStatus string
	}{
		{
			name:       "ready by default",
			checker:    nil,
			wantCode:   http.StatusOK,
			wantStatus: "ready",
		},
		{
			name: "not ready when checker fails",
			checker: ReadinessCheckerFunc(func(context.Context) error {
				return errors.New("dependency unavailable")
			}),
			wantCode:   http.StatusServiceUnavailable,
			wantStatus: "not_ready",
		},
	}

	for _, tt := range tests {
		tt := tt
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()

			req := httptest.NewRequest(http.MethodGet, "/readyz", nil)
			rec := httptest.NewRecorder()
			readyzHandler := NewReadyzWebHandler(tt.checker)

			readyzHandler.Readyz(rec, req)

			if rec.Code != tt.wantCode {
				t.Fatalf("expected status %d, got %d", tt.wantCode, rec.Code)
			}

			if got := rec.Header().Get("Content-Type"); got != contentTypeJSON {
				t.Fatalf("expected content type %q, got %q", contentTypeJSON, got)
			}

			var response statusResponse
			if err := json.NewDecoder(rec.Body).Decode(&response); err != nil {
				t.Fatalf("decode response: %v", err)
			}

			if response.Status != tt.wantStatus {
				t.Fatalf("expected status %q, got %q", tt.wantStatus, response.Status)
			}
		})
	}
}
