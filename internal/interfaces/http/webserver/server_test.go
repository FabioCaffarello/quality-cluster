package webserver

import (
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"internal/shared/settings"
)

func TestNewWebServerBuildsHTTPServerWithConfig(t *testing.T) {
	t.Parallel()

	config := settings.HTTPConfig{
		Addr:            ":8080",
		ReadTimeout:     "2s",
		WriteTimeout:    "3s",
		IdleTimeout:     "4s",
		ShutdownTimeout: "5s",
	}

	server := NewWebServer(config)

	if server.server.Addr != config.Addr {
		t.Fatalf("expected addr %q, got %q", config.Addr, server.server.Addr)
	}

	if server.server.Handler != server.router {
		t.Fatal("expected server handler to use the router")
	}

	if server.server.ReadTimeout != 2*time.Second {
		t.Fatalf("expected read timeout %s, got %s", 2*time.Second, server.server.ReadTimeout)
	}

	if server.server.WriteTimeout != 3*time.Second {
		t.Fatalf("expected write timeout %s, got %s", 3*time.Second, server.server.WriteTimeout)
	}

	if server.server.IdleTimeout != 4*time.Second {
		t.Fatalf("expected idle timeout %s, got %s", 4*time.Second, server.server.IdleTimeout)
	}
}

func TestRegisterRoutes(t *testing.T) {
	t.Parallel()

	server := NewWebServer(settings.HTTPConfig{Addr: ":0"})
	server.RegisterRoutes([]Route{
		{
			Method: http.MethodGet,
			Path:   "/ping",
			Handler: func(w http.ResponseWriter, _ *http.Request) {
				w.WriteHeader(http.StatusNoContent)
			},
		},
	})

	req := httptest.NewRequest(http.MethodGet, "/ping", nil)
	rec := httptest.NewRecorder()

	server.server.Handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusNoContent {
		t.Fatalf("expected status %d, got %d", http.StatusNoContent, rec.Code)
	}
}
