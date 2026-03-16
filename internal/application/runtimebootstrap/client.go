package runtimebootstrap

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"net/url"
	"sort"
	"strconv"
	"strings"
	"time"

	configctlcontracts "internal/application/configctl/contracts"
	dataplaneapp "internal/application/dataplane"
	sharedruntime "internal/application/runtimecontracts"
	"internal/shared/problem"
	"internal/shared/requestctx"
)

const DefaultPollInterval = 5 * time.Second

type Client struct {
	baseURL    string
	httpClient *http.Client
}

type WaitOptions struct {
	ScopeKind     string
	ScopeKey      string
	CorrelationID string
	PollInterval  time.Duration
}

type AggregateWaitOptions struct {
	CorrelationID string
	PollInterval  time.Duration
}

type ActiveIngestionBootstrap struct {
	Bindings []configctlcontracts.ActiveIngestionBindingRecord
	Runtimes []sharedruntime.RuntimeRecord
	Index    dataplaneapp.BindingIndex
	Topology dataplaneapp.RuntimeTopology
}

func (b ActiveIngestionBootstrap) Signature() string {
	bindings := append([]configctlcontracts.ActiveIngestionBindingRecord(nil), b.Bindings...)
	runtimes := canonicalBootstrapRuntimes(b.Runtimes, bindings)
	if len(bindings) == 0 && len(runtimes) == 0 {
		return ""
	}

	parts := make([]string, 0, len(bindings)+len(runtimes))
	for _, runtime := range runtimes {
		parts = append(parts, strings.Join([]string{
			"runtime",
			strings.TrimSpace(runtime.Scope.Kind),
			strings.TrimSpace(runtime.Scope.Key),
			strings.TrimSpace(runtime.Config.SetID),
			strings.TrimSpace(runtime.Config.Key),
			strings.TrimSpace(runtime.Config.VersionID),
			strconv.Itoa(runtime.Config.Version),
			strings.TrimSpace(runtime.Config.DefinitionChecksum),
			strings.TrimSpace(runtime.Artifact.ID),
			strings.TrimSpace(runtime.Artifact.SchemaVersion),
			strings.TrimSpace(runtime.Artifact.Checksum),
			strings.TrimSpace(runtime.Artifact.StorageRef),
			strings.TrimSpace(runtime.Artifact.RuntimeLoader),
		}, "|"))
	}
	for _, binding := range bindings {
		parts = append(parts, strings.Join([]string{
			"binding",
			strings.TrimSpace(binding.Runtime.Scope.Kind),
			strings.TrimSpace(binding.Runtime.Scope.Key),
			strings.TrimSpace(binding.Runtime.Config.SetID),
			strings.TrimSpace(binding.Runtime.Config.Key),
			strings.TrimSpace(binding.Runtime.Config.VersionID),
			strconv.Itoa(binding.Runtime.Config.Version),
			strings.TrimSpace(binding.Runtime.Config.DefinitionChecksum),
			strings.TrimSpace(binding.Runtime.Artifact.ID),
			strings.TrimSpace(binding.Runtime.Artifact.SchemaVersion),
			strings.TrimSpace(binding.Runtime.Artifact.Checksum),
			strings.TrimSpace(binding.Runtime.Artifact.StorageRef),
			strings.TrimSpace(binding.Runtime.Artifact.RuntimeLoader),
			strings.TrimSpace(binding.Binding.Name),
			strings.TrimSpace(binding.Binding.Topic),
		}, "|"))
	}
	sort.Strings(parts)
	return strings.Join(parts, "\n")
}

func (b ActiveIngestionBootstrap) RuntimeRefs() []string {
	runtimes := canonicalBootstrapRuntimes(b.Runtimes, b.Bindings)
	refs := make([]string, 0, len(runtimes))
	for _, runtime := range runtimes {
		refs = append(refs, strings.Join([]string{
			strings.TrimSpace(runtime.Scope.Kind),
			strings.TrimSpace(runtime.Scope.Key),
			strings.TrimSpace(runtime.Config.VersionID),
			strings.TrimSpace(runtime.Artifact.ID),
		}, ":"))
	}
	return refs
}

func NewClient(baseURL string, timeout time.Duration) *Client {
	return &Client{
		baseURL: strings.TrimRight(strings.TrimSpace(baseURL), "/"),
		httpClient: &http.Client{
			Timeout: timeout,
		},
	}
}

func (c *Client) ListActiveIngestionBindings(ctx context.Context, query configctlcontracts.ListActiveIngestionBindingsQuery) (configctlcontracts.ListActiveIngestionBindingsReply, *problem.Problem) {
	if c == nil || strings.TrimSpace(c.baseURL) == "" {
		return configctlcontracts.ListActiveIngestionBindingsReply{}, problem.New(problem.Unavailable, "runtime bootstrap client is unavailable")
	}

	query = query.Normalize()
	values := url.Values{}
	if query.ScopeKind != "" {
		values.Set("scope_kind", query.ScopeKind)
	}
	if query.ScopeKey != "" {
		values.Set("scope_key", query.ScopeKey)
	}

	endpoint := c.baseURL + "/runtime/ingestion/bindings"
	if encoded := values.Encode(); encoded != "" {
		endpoint += "?" + encoded
	}

	request, err := http.NewRequestWithContext(ctx, http.MethodGet, endpoint, nil)
	if err != nil {
		return configctlcontracts.ListActiveIngestionBindingsReply{}, problem.Wrap(err, problem.Internal, "build runtime bootstrap request")
	}
	request.Header.Set("Accept", "application/json")
	if correlationID := requestctx.CorrelationID(ctx); correlationID != "" {
		request.Header.Set("X-Correlation-ID", correlationID)
	}

	response, err := c.httpClient.Do(request)
	if err != nil {
		return configctlcontracts.ListActiveIngestionBindingsReply{}, problem.Wrap(err, problem.Unavailable, "execute runtime bootstrap request")
	}
	defer response.Body.Close()

	if response.StatusCode >= http.StatusBadRequest {
		var prob problem.Problem
		if err := json.NewDecoder(response.Body).Decode(&prob); err == nil && prob.Code != "" {
			return configctlcontracts.ListActiveIngestionBindingsReply{}, &prob
		}
		return configctlcontracts.ListActiveIngestionBindingsReply{}, problem.New(problem.Unavailable, fmt.Sprintf("runtime bootstrap request failed with status %d", response.StatusCode))
	}

	var reply configctlcontracts.ListActiveIngestionBindingsReply
	if err := json.NewDecoder(response.Body).Decode(&reply); err != nil {
		return configctlcontracts.ListActiveIngestionBindingsReply{}, problem.Wrap(err, problem.Internal, "decode runtime bootstrap response")
	}

	return reply, nil
}

func (c *Client) WaitForActiveIngestionBootstrap(ctx context.Context, logger *slog.Logger, options WaitOptions) (ActiveIngestionBootstrap, *problem.Problem) {
	options = options.normalize()
	query := configctlcontracts.ListActiveIngestionBindingsQuery{
		ScopeKind: options.ScopeKind,
		ScopeKey:  options.ScopeKey,
	}

	return c.waitForActiveIngestionBootstrap(ctx, logger, query, options.CorrelationID, options.PollInterval)
}

func (c *Client) WaitForActiveIngestionBootstrapSet(ctx context.Context, logger *slog.Logger, options AggregateWaitOptions) (ActiveIngestionBootstrap, *problem.Problem) {
	options = options.normalize()
	return c.waitForActiveIngestionBootstrap(ctx, logger, configctlcontracts.ListActiveIngestionBindingsQuery{}, options.CorrelationID, options.PollInterval)
}

func (c *Client) waitForActiveIngestionBootstrap(ctx context.Context, logger *slog.Logger, query configctlcontracts.ListActiveIngestionBindingsQuery, correlationID string, pollInterval time.Duration) (ActiveIngestionBootstrap, *problem.Problem) {
	for {
		reply, prob := c.ListActiveIngestionBindings(requestctx.WithCorrelationID(ctx, correlationID), query)
		if prob == nil && len(reply.Bindings) > 0 {
			return buildActiveIngestionBootstrap(reply)
		}

		switch {
		case ctx.Err() != nil:
			return ActiveIngestionBootstrap{}, problem.Wrap(ctx.Err(), problem.Unavailable, "runtime bootstrap was interrupted")
		case prob != nil:
			logWait(logger, "waiting for active ingestion bindings", "error", prob)
		default:
			logWait(logger, "waiting for active ingestion bindings", "reason", "none active yet")
		}

		timer := time.NewTimer(pollInterval)
		select {
		case <-ctx.Done():
			timer.Stop()
			return ActiveIngestionBootstrap{}, problem.Wrap(ctx.Err(), problem.Unavailable, "runtime bootstrap was interrupted")
		case <-timer.C:
		}
	}
}

func buildActiveIngestionBootstrap(reply configctlcontracts.ListActiveIngestionBindingsReply) (ActiveIngestionBootstrap, *problem.Problem) {
	runtimes, prob := validateBootstrapRuntimes(reply.Bindings, reply.Runtimes)
	if prob != nil {
		return ActiveIngestionBootstrap{}, prob
	}

	index, indexProb := dataplaneapp.NewBindingIndex(reply.Bindings)
	if indexProb != nil {
		return ActiveIngestionBootstrap{}, indexProb
	}
	topology, topologyProb := dataplaneapp.NewRuntimeTopology(index, dataplaneapp.DefaultRegistry())
	if topologyProb != nil {
		return ActiveIngestionBootstrap{}, topologyProb
	}

	return ActiveIngestionBootstrap{
		Bindings: reply.Bindings,
		Runtimes: runtimes,
		Index:    index,
		Topology: topology,
	}, nil
}

func validateBootstrapRuntimes(bindings []configctlcontracts.ActiveIngestionBindingRecord, runtimes []sharedruntime.RuntimeRecord) ([]sharedruntime.RuntimeRecord, *problem.Problem) {
	if len(bindings) == 0 {
		return nil, nil
	}
	if len(runtimes) == 0 {
		return nil, problem.New(problem.InvalidArgument, "active ingestion bootstrap must include compact runtimes")
	}

	canonical := canonicalBootstrapRuntimes(runtimes, nil)
	runtimeSet := make(map[string]struct{}, len(canonical))
	runtimeRefs := make(map[string]int, len(canonical))
	for _, runtime := range canonical {
		key, issues := runtimeIdentityKey(runtime, "runtimes")
		if len(issues) > 0 {
			return nil, problem.Validation(problem.InvalidArgument, "active ingestion bootstrap runtime summary is invalid", issues...)
		}
		runtimeSet[key] = struct{}{}
	}

	for _, binding := range bindings {
		key, issues := runtimeIdentityKey(binding.Runtime, "bindings[].runtime")
		if len(issues) > 0 {
			return nil, problem.Validation(problem.InvalidArgument, "active ingestion bootstrap binding runtime is invalid", issues...)
		}
		if _, ok := runtimeSet[key]; !ok {
			return nil, problem.New(problem.Conflict, "active ingestion bootstrap runtimes do not match binding runtime state")
		}
		runtimeRefs[key]++
	}

	for _, runtime := range canonical {
		key, _ := runtimeIdentityKey(runtime, "runtimes")
		if runtimeRefs[key] == 0 {
			return nil, problem.New(problem.Conflict, "active ingestion bootstrap contains runtime summaries without active bindings")
		}
	}

	return canonical, nil
}

func canonicalBootstrapRuntimes(runtimes []sharedruntime.RuntimeRecord, bindings []configctlcontracts.ActiveIngestionBindingRecord) []sharedruntime.RuntimeRecord {
	if len(runtimes) == 0 && len(bindings) > 0 {
		seen := make(map[string]struct{}, len(bindings))
		runtimes = make([]sharedruntime.RuntimeRecord, 0, len(bindings))
		for _, binding := range bindings {
			key, _ := runtimeIdentityKey(binding.Runtime, "bindings[].runtime")
			if _, ok := seen[key]; ok {
				continue
			}
			seen[key] = struct{}{}
			runtimes = append(runtimes, binding.Runtime)
		}
	}

	canonical := append([]sharedruntime.RuntimeRecord(nil), runtimes...)
	sort.SliceStable(canonical, func(i, j int) bool {
		left := canonical[i]
		right := canonical[j]
		if left.Scope.Kind != right.Scope.Kind {
			return left.Scope.Kind < right.Scope.Kind
		}
		if left.Scope.Key != right.Scope.Key {
			return left.Scope.Key < right.Scope.Key
		}
		if left.Config.VersionID != right.Config.VersionID {
			return left.Config.VersionID < right.Config.VersionID
		}
		if left.Config.DefinitionChecksum != right.Config.DefinitionChecksum {
			return left.Config.DefinitionChecksum < right.Config.DefinitionChecksum
		}
		if left.Artifact.ID != right.Artifact.ID {
			return left.Artifact.ID < right.Artifact.ID
		}
		return left.Artifact.Checksum < right.Artifact.Checksum
	})

	return canonical
}

func runtimeIdentityKey(runtime sharedruntime.RuntimeRecord, fieldRoot string) (string, []problem.ValidationIssue) {
	scopeKind := strings.TrimSpace(runtime.Scope.Kind)
	scopeKey := strings.TrimSpace(runtime.Scope.Key)
	versionID := strings.TrimSpace(runtime.Config.VersionID)
	definitionChecksum := strings.TrimSpace(runtime.Config.DefinitionChecksum)
	artifactID := strings.TrimSpace(runtime.Artifact.ID)
	artifactChecksum := strings.TrimSpace(runtime.Artifact.Checksum)
	runtimeLoader := strings.TrimSpace(runtime.Artifact.RuntimeLoader)

	var issues []problem.ValidationIssue
	if scopeKind == "" {
		issues = append(issues, problem.ValidationIssue{Field: fieldRoot + ".scope.kind", Message: "must not be empty"})
	}
	if scopeKey == "" {
		issues = append(issues, problem.ValidationIssue{Field: fieldRoot + ".scope.key", Message: "must not be empty"})
	}
	if versionID == "" {
		issues = append(issues, problem.ValidationIssue{Field: fieldRoot + ".config.version_id", Message: "must not be empty"})
	}
	if definitionChecksum == "" {
		issues = append(issues, problem.ValidationIssue{Field: fieldRoot + ".config.definition_checksum", Message: "must not be empty"})
	}
	if artifactID == "" {
		issues = append(issues, problem.ValidationIssue{Field: fieldRoot + ".artifact.id", Message: "must not be empty"})
	}
	if artifactChecksum == "" {
		issues = append(issues, problem.ValidationIssue{Field: fieldRoot + ".artifact.checksum", Message: "must not be empty"})
	}
	if runtimeLoader == "" {
		issues = append(issues, problem.ValidationIssue{Field: fieldRoot + ".artifact.runtime_loader", Message: "must not be empty"})
	}
	if len(issues) > 0 {
		return "", issues
	}

	return strings.Join([]string{
		scopeKind,
		scopeKey,
		strings.TrimSpace(runtime.Config.SetID),
		strings.TrimSpace(runtime.Config.Key),
		versionID,
		strconv.Itoa(runtime.Config.Version),
		definitionChecksum,
		artifactID,
		strings.TrimSpace(runtime.Artifact.SchemaVersion),
		artifactChecksum,
		strings.TrimSpace(runtime.Artifact.StorageRef),
		runtimeLoader,
	}, "|"), nil
}

func (o WaitOptions) normalize() WaitOptions {
	o.ScopeKind = strings.ToLower(strings.TrimSpace(o.ScopeKind))
	o.ScopeKey = strings.TrimSpace(o.ScopeKey)
	if o.ScopeKind == "" {
		o.ScopeKind = "global"
	}
	if o.ScopeKey == "" {
		o.ScopeKey = "default"
	}
	if o.PollInterval <= 0 {
		o.PollInterval = DefaultPollInterval
	}
	return o
}

func (o AggregateWaitOptions) normalize() AggregateWaitOptions {
	if o.PollInterval <= 0 {
		o.PollInterval = DefaultPollInterval
	}
	return o
}

func logWait(logger *slog.Logger, msg string, args ...any) {
	if logger == nil {
		return
	}
	logger.Info(msg, args...)
}
