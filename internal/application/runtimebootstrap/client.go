package runtimebootstrap

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"net/url"
	"strings"
	"time"

	configctlcontracts "internal/application/configctl/contracts"
	dataplaneapp "internal/application/dataplane"
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

type ActiveIngestionBootstrap struct {
	Bindings []configctlcontracts.ActiveIngestionBindingRecord
	Index    dataplaneapp.BindingIndex
	Topology dataplaneapp.RuntimeTopology
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

	for {
		reply, prob := c.ListActiveIngestionBindings(requestctx.WithCorrelationID(ctx, options.CorrelationID), query)
		if prob == nil && len(reply.Bindings) > 0 {
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
				Index:    index,
				Topology: topology,
			}, nil
		}

		switch {
		case ctx.Err() != nil:
			return ActiveIngestionBootstrap{}, problem.Wrap(ctx.Err(), problem.Unavailable, "runtime bootstrap was interrupted")
		case prob != nil:
			logWait(logger, "waiting for active ingestion bindings", "error", prob)
		default:
			logWait(logger, "waiting for active ingestion bindings", "reason", "none active yet")
		}

		timer := time.NewTimer(options.PollInterval)
		select {
		case <-ctx.Done():
			timer.Stop()
			return ActiveIngestionBootstrap{}, problem.Wrap(ctx.Err(), problem.Unavailable, "runtime bootstrap was interrupted")
		case <-timer.C:
		}
	}
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

func logWait(logger *slog.Logger, msg string, args ...any) {
	if logger == nil {
		return
	}
	logger.Info(msg, args...)
}
