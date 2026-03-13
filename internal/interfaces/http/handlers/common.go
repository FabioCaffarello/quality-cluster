package handlers

import (
	"encoding/json"
	"net/http"

	"internal/shared/problem"
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

func writeJSONResponse(w http.ResponseWriter, code int, value any) {
	w.Header().Set("Content-Type", contentTypeJSON)
	w.WriteHeader(code)
	_ = json.NewEncoder(w).Encode(value)
}

func writeProblemResponse(w http.ResponseWriter, prob *problem.Problem) {
	if prob == nil {
		prob = problem.New(problem.Internal, "unexpected error")
	}

	writeJSONResponse(w, problemHTTPStatus(prob), prob)
}

func problemHTTPStatus(prob *problem.Problem) int {
	switch prob.Code {
	case problem.InvalidArgument, problem.ValidationFailed:
		return http.StatusBadRequest
	case problem.NotFound:
		return http.StatusNotFound
	case problem.Conflict:
		return http.StatusConflict
	case problem.Unavailable:
		return http.StatusServiceUnavailable
	default:
		return http.StatusInternalServerError
	}
}
