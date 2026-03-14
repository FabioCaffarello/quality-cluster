package handlers

import (
	"context"
	"net/http"
	"strconv"

	configctlcontracts "internal/application/configctl/contracts"
	validatorresultscontracts "internal/application/validatorresults/contracts"
	runtimecontracts "internal/application/validatorruntime/contracts"
	"internal/shared/problem"
)

type getValidatorRuntimeUseCase interface {
	Execute(context.Context, runtimecontracts.GetActiveRuntimeQuery) (runtimecontracts.GetActiveRuntimeReply, *problem.Problem)
}

type RuntimeWebHandler struct {
	getActiveRuntime            getValidatorRuntimeUseCase
	listActiveIngestionBindings listActiveIngestionBindingsUseCase
	listValidationResults       listValidationResultsUseCase
}

type listActiveIngestionBindingsUseCase interface {
	Execute(context.Context, configctlcontracts.ListActiveIngestionBindingsQuery) (configctlcontracts.ListActiveIngestionBindingsReply, *problem.Problem)
}

type listValidationResultsUseCase interface {
	Execute(context.Context, validatorresultscontracts.ListValidationResultsQuery) (validatorresultscontracts.ListValidationResultsReply, *problem.Problem)
}

func NewRuntimeWebHandler(getActiveRuntime getValidatorRuntimeUseCase, listActiveIngestionBindings listActiveIngestionBindingsUseCase, listValidationResults listValidationResultsUseCase) *RuntimeWebHandler {
	return &RuntimeWebHandler{
		getActiveRuntime:            getActiveRuntime,
		listActiveIngestionBindings: listActiveIngestionBindings,
		listValidationResults:       listValidationResults,
	}
}

func (h *RuntimeWebHandler) GetActiveValidatorRuntime(w http.ResponseWriter, r *http.Request) {
	if h == nil || h.getActiveRuntime == nil {
		writeProblemResponse(w, problem.New(problem.Unavailable, "validator runtime lookup is unavailable"))
		return
	}

	reply, prob := h.getActiveRuntime.Execute(withCorrelationID(r), runtimecontracts.GetActiveRuntimeQuery{
		ScopeKind: r.URL.Query().Get("scope_kind"),
		ScopeKey:  r.URL.Query().Get("scope_key"),
	})
	if prob != nil {
		writeProblemResponse(w, prob)
		return
	}

	writeJSONResponse(w, http.StatusOK, reply)
}

func (h *RuntimeWebHandler) ListActiveIngestionBindings(w http.ResponseWriter, r *http.Request) {
	if h == nil || h.listActiveIngestionBindings == nil {
		writeProblemResponse(w, problem.New(problem.Unavailable, "ingestion runtime lookup is unavailable"))
		return
	}

	reply, prob := h.listActiveIngestionBindings.Execute(withCorrelationID(r), configctlcontracts.ListActiveIngestionBindingsQuery{
		ScopeKind: r.URL.Query().Get("scope_kind"),
		ScopeKey:  r.URL.Query().Get("scope_key"),
	})
	if prob != nil {
		writeProblemResponse(w, prob)
		return
	}

	writeJSONResponse(w, http.StatusOK, reply)
}

func (h *RuntimeWebHandler) ListValidationResults(w http.ResponseWriter, r *http.Request) {
	if h == nil || h.listValidationResults == nil {
		writeProblemResponse(w, problem.New(problem.Unavailable, "validation results lookup is unavailable"))
		return
	}

	limit, err := strconv.Atoi(r.URL.Query().Get("limit"))
	if r.URL.Query().Get("limit") != "" && err != nil {
		writeProblemResponse(w, problem.New(problem.InvalidArgument, "limit must be a valid integer"))
		return
	}

	reply, prob := h.listValidationResults.Execute(withCorrelationID(r), validatorresultscontracts.ListValidationResultsQuery{
		ScopeKind:     r.URL.Query().Get("scope_kind"),
		ScopeKey:      r.URL.Query().Get("scope_key"),
		BindingName:   r.URL.Query().Get("binding_name"),
		Topic:         r.URL.Query().Get("topic"),
		MessageID:     r.URL.Query().Get("message_id"),
		CorrelationID: r.URL.Query().Get("correlation_id"),
		Limit:         limit,
	})
	if prob != nil {
		writeProblemResponse(w, prob)
		return
	}

	writeJSONResponse(w, http.StatusOK, reply)
}
