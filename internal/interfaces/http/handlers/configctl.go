package handlers

import (
	"context"
	"encoding/json"
	"io"
	"net/http"

	configctlcontracts "internal/application/configctl/contracts"
	"internal/shared/problem"
	"internal/shared/requestctx"

	"github.com/julienschmidt/httprouter"
)

type createDraftUseCase interface {
	Execute(context.Context, configctlcontracts.CreateDraftCommand) (configctlcontracts.CreateDraftReply, *problem.Problem)
}

type getConfigUseCase interface {
	Execute(context.Context, configctlcontracts.GetConfigQuery) (configctlcontracts.GetConfigReply, *problem.Problem)
}

type getActiveConfigUseCase interface {
	Execute(context.Context, configctlcontracts.GetActiveConfigQuery) (configctlcontracts.GetActiveConfigReply, *problem.Problem)
}

type listConfigsUseCase interface {
	Execute(context.Context, configctlcontracts.ListConfigsQuery) (configctlcontracts.ListConfigsReply, *problem.Problem)
}

type validateDraftUseCase interface {
	Execute(context.Context, configctlcontracts.ValidateDraftCommand) (configctlcontracts.ValidateDraftReply, *problem.Problem)
}

type validateConfigUseCase interface {
	Execute(context.Context, configctlcontracts.ValidateConfigCommand) (configctlcontracts.ValidateConfigReply, *problem.Problem)
}

type compileConfigUseCase interface {
	Execute(context.Context, configctlcontracts.CompileConfigCommand) (configctlcontracts.CompileConfigReply, *problem.Problem)
}

type activateConfigUseCase interface {
	Execute(context.Context, configctlcontracts.ActivateConfigCommand) (configctlcontracts.ActivateConfigReply, *problem.Problem)
}

type ConfigctlWebHandler struct {
	createDraft    createDraftUseCase
	getConfig      getConfigUseCase
	getActive      getActiveConfigUseCase
	listConfigs    listConfigsUseCase
	validateDraft  validateDraftUseCase
	validateConfig validateConfigUseCase
	compileConfig  compileConfigUseCase
	activateConfig activateConfigUseCase
}

type createDraftRequest struct {
	Name    string `json:"name"`
	Format  string `json:"format"`
	Content string `json:"content"`
}

type createDraftResponse struct {
	Status string                                 `json:"status"`
	Config configctlcontracts.ConfigVersionDetail `json:"config"`
}

type getConfigResponse struct {
	Config configctlcontracts.ConfigVersionDetail `json:"config"`
}

type listConfigsResponse struct {
	Configs []configctlcontracts.ConfigVersionSummary `json:"configs"`
}

type validateDraftRequest struct {
	Format  string `json:"format"`
	Content string `json:"content"`
}

type validateDraftResponse struct {
	Status     string                                `json:"status"`
	Validation configctlcontracts.ValidateDraftReply `json:"validation"`
}

type validateConfigResponse struct {
	Status     string                                 `json:"status"`
	Validation configctlcontracts.ValidateConfigReply `json:"validation"`
}

type compileConfigRequest struct {
	ArtifactID      string `json:"artifact_id,omitempty"`
	SchemaVersion   string `json:"schema_version,omitempty"`
	Checksum        string `json:"checksum,omitempty"`
	StorageRef      string `json:"storage_ref,omitempty"`
	RuntimeLoader   string `json:"runtime_loader,omitempty"`
	CompilerVersion string `json:"compiler_version,omitempty"`
}

type compileConfigResponse struct {
	Status string                                 `json:"status"`
	Config configctlcontracts.ConfigVersionDetail `json:"config"`
}

type activateConfigRequest struct {
	ScopeKind string `json:"scope_kind,omitempty"`
	ScopeKey  string `json:"scope_key,omitempty"`
}

type activateConfigResponse struct {
	Status     string                                     `json:"status"`
	Config     configctlcontracts.ConfigVersionDetail     `json:"config"`
	Activation configctlcontracts.ActivationRecord        `json:"activation"`
	Projection configctlcontracts.RuntimeProjectionRecord `json:"projection"`
}

func NewConfigctlWebHandler(
	createDraft createDraftUseCase,
	getConfig getConfigUseCase,
	getActive getActiveConfigUseCase,
	listConfigs listConfigsUseCase,
	validateDraft validateDraftUseCase,
	validateConfig validateConfigUseCase,
	compileConfig compileConfigUseCase,
	activateConfig activateConfigUseCase,
) *ConfigctlWebHandler {
	return &ConfigctlWebHandler{
		createDraft:    createDraft,
		getConfig:      getConfig,
		getActive:      getActive,
		listConfigs:    listConfigs,
		validateDraft:  validateDraft,
		validateConfig: validateConfig,
		compileConfig:  compileConfig,
		activateConfig: activateConfig,
	}
}

func (h *ConfigctlWebHandler) CreateDraft(w http.ResponseWriter, r *http.Request) {
	if h == nil || h.createDraft == nil {
		writeProblemResponse(w, problem.New(problem.Unavailable, "draft creation is unavailable"))
		return
	}

	request, prob := decodeJSONBody[createDraftRequest](r)
	if prob != nil {
		writeProblemResponse(w, prob)
		return
	}

	ctx := requestctx.WithCorrelationID(r.Context(), r.Header.Get("X-Correlation-ID"))
	result, execProb := h.createDraft.Execute(ctx, configctlcontracts.CreateDraftCommand{
		Name:    request.Name,
		Format:  request.Format,
		Content: request.Content,
	})
	if execProb != nil {
		writeProblemResponse(w, execProb)
		return
	}

	writeJSONResponse(w, http.StatusCreated, createDraftResponse{
		Status: "created",
		Config: result.Config,
	})
}

func (h *ConfigctlWebHandler) GetConfig(w http.ResponseWriter, r *http.Request) {
	if h == nil || h.getConfig == nil {
		writeProblemResponse(w, problem.New(problem.Unavailable, "config lookup is unavailable"))
		return
	}

	result, prob := h.getConfig.Execute(withCorrelationID(r), configctlcontracts.GetConfigQuery{
		VersionID: versionIDFromRequest(r),
	})
	if prob != nil {
		writeProblemResponse(w, prob)
		return
	}

	writeJSONResponse(w, http.StatusOK, getConfigResponse{Config: result.Config})
}

func (h *ConfigctlWebHandler) GetActiveConfig(w http.ResponseWriter, r *http.Request) {
	if h == nil || h.getActive == nil {
		writeProblemResponse(w, problem.New(problem.Unavailable, "active config lookup is unavailable"))
		return
	}

	result, prob := h.getActive.Execute(withCorrelationID(r), configctlcontracts.GetActiveConfigQuery{
		ScopeKind: r.URL.Query().Get("scope_kind"),
		ScopeKey:  r.URL.Query().Get("scope_key"),
	})
	if prob != nil {
		writeProblemResponse(w, prob)
		return
	}

	writeJSONResponse(w, http.StatusOK, getConfigResponse{Config: result.Config})
}

func (h *ConfigctlWebHandler) ListConfigs(w http.ResponseWriter, r *http.Request) {
	if h == nil || h.listConfigs == nil {
		writeProblemResponse(w, problem.New(problem.Unavailable, "config listing is unavailable"))
		return
	}

	result, prob := h.listConfigs.Execute(withCorrelationID(r), configctlcontracts.ListConfigsQuery{})
	if prob != nil {
		writeProblemResponse(w, prob)
		return
	}

	writeJSONResponse(w, http.StatusOK, listConfigsResponse{Configs: result.Configs})
}

func (h *ConfigctlWebHandler) ValidateDraft(w http.ResponseWriter, r *http.Request) {
	if h == nil || h.validateDraft == nil {
		writeProblemResponse(w, problem.New(problem.Unavailable, "draft validation is unavailable"))
		return
	}

	request, prob := decodeJSONBody[validateDraftRequest](r)
	if prob != nil {
		writeProblemResponse(w, prob)
		return
	}

	result, execProb := h.validateDraft.Execute(withCorrelationID(r), configctlcontracts.ValidateDraftCommand{
		Format:  request.Format,
		Content: request.Content,
	})
	if execProb != nil {
		writeProblemResponse(w, execProb)
		return
	}

	status := "invalid"
	if result.Valid {
		status = "valid"
	}
	writeJSONResponse(w, http.StatusOK, validateDraftResponse{
		Status:     status,
		Validation: result,
	})
}

func (h *ConfigctlWebHandler) ValidateConfig(w http.ResponseWriter, r *http.Request) {
	if h == nil || h.validateConfig == nil {
		writeProblemResponse(w, problem.New(problem.Unavailable, "config validation is unavailable"))
		return
	}

	result, prob := h.validateConfig.Execute(withCorrelationID(r), configctlcontracts.ValidateConfigCommand{
		VersionID: versionIDFromRequest(r),
	})
	if prob != nil {
		writeProblemResponse(w, prob)
		return
	}

	status := "invalid"
	if result.Valid {
		status = "validated"
	}
	writeJSONResponse(w, http.StatusOK, validateConfigResponse{
		Status:     status,
		Validation: result,
	})
}

func (h *ConfigctlWebHandler) CompileConfig(w http.ResponseWriter, r *http.Request) {
	if h == nil || h.compileConfig == nil {
		writeProblemResponse(w, problem.New(problem.Unavailable, "config compilation is unavailable"))
		return
	}

	request, prob := decodeOptionalJSONBody[compileConfigRequest](r)
	if prob != nil {
		writeProblemResponse(w, prob)
		return
	}

	result, execProb := h.compileConfig.Execute(withCorrelationID(r), configctlcontracts.CompileConfigCommand{
		VersionID:       versionIDFromRequest(r),
		ArtifactID:      request.ArtifactID,
		SchemaVersion:   request.SchemaVersion,
		Checksum:        request.Checksum,
		StorageRef:      request.StorageRef,
		RuntimeLoader:   request.RuntimeLoader,
		CompilerVersion: request.CompilerVersion,
	})
	if execProb != nil {
		writeProblemResponse(w, execProb)
		return
	}

	writeJSONResponse(w, http.StatusOK, compileConfigResponse{
		Status: "compiled",
		Config: result.Config,
	})
}

func (h *ConfigctlWebHandler) ActivateConfig(w http.ResponseWriter, r *http.Request) {
	if h == nil || h.activateConfig == nil {
		writeProblemResponse(w, problem.New(problem.Unavailable, "config activation is unavailable"))
		return
	}

	request, prob := decodeOptionalJSONBody[activateConfigRequest](r)
	if prob != nil {
		writeProblemResponse(w, prob)
		return
	}

	result, execProb := h.activateConfig.Execute(withCorrelationID(r), configctlcontracts.ActivateConfigCommand{
		VersionID: versionIDFromRequest(r),
		ScopeKind: request.ScopeKind,
		ScopeKey:  request.ScopeKey,
	})
	if execProb != nil {
		writeProblemResponse(w, execProb)
		return
	}

	writeJSONResponse(w, http.StatusOK, activateConfigResponse{
		Status:     "activated",
		Config:     result.Config,
		Activation: result.Activation,
		Projection: result.Projection,
	})
}

func decodeJSONBody[T any](r *http.Request) (T, *problem.Problem) {
	var zero T
	defer r.Body.Close()

	decoder := json.NewDecoder(r.Body)
	decoder.DisallowUnknownFields()

	var request T
	if err := decoder.Decode(&request); err != nil {
		return zero, problem.Wrap(err, problem.InvalidArgument, "request body must be valid JSON")
	}

	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		return zero, problem.New(problem.InvalidArgument, "request body must contain a single JSON object")
	}

	return request, nil
}

func decodeOptionalJSONBody[T any](r *http.Request) (T, *problem.Problem) {
	var zero T
	if r == nil || r.Body == nil {
		return zero, nil
	}
	defer r.Body.Close()

	decoder := json.NewDecoder(r.Body)
	decoder.DisallowUnknownFields()

	var request T
	if err := decoder.Decode(&request); err != nil {
		if err == io.EOF {
			return zero, nil
		}
		return zero, problem.Wrap(err, problem.InvalidArgument, "request body must be valid JSON")
	}

	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		return zero, problem.New(problem.InvalidArgument, "request body must contain a single JSON object")
	}

	return request, nil
}

func pathParam(r *http.Request, name string) string {
	return httprouter.ParamsFromContext(r.Context()).ByName(name)
}

func versionIDFromRequest(r *http.Request) string {
	if id := pathParam(r, "id"); id != "" {
		return id
	}
	return r.URL.Query().Get("id")
}

func withCorrelationID(r *http.Request) context.Context {
	return requestctx.WithCorrelationID(r.Context(), r.Header.Get("X-Correlation-ID"))
}
