package settings

import (
	"fmt"
	"internal/shared/problem"
	"strings"
	"time"
)

type AppConfig struct {
	Log       LogConfig       `json:"log"`
	HTTP      HTTPConfig      `json:"http"`
	NATS      NATSConfig      `json:"nats"`
	Kafka     KafkaConfig     `json:"kafka"`
	Bootstrap BootstrapConfig `json:"bootstrap"`
	Emulator  EmulatorConfig  `json:"emulator"`
}

// Defaults returns the baseline shared application config.
func Defaults() AppConfig {
	return AppConfig{
		Log: LogConfig{
			Level:  LogLevelInfo,
			Format: LogFormatText,
		},
		HTTP: HTTPConfig{
			Addr:            ":8080",
			ReadTimeout:     "10s",
			WriteTimeout:    "15s",
			IdleTimeout:     "60s",
			ShutdownTimeout: "10s",
		},
		NATS: NATSConfig{
			RequestTimeout: "2s",
		},
		Kafka: KafkaConfig{
			DialTimeout: "10s",
		},
		Bootstrap: BootstrapConfig{
			ScopeKind: "global",
			ScopeKey:  "default",
			Timeout:   "5s",
		},
		Emulator: EmulatorConfig{
			PublishInterval: "5s",
		},
	}
}

// ApplyDefaults fills empty fields with the package defaults.
func (c *AppConfig) ApplyDefaults() {
	if c == nil {
		return
	}

	defaults := Defaults()
	c.Log.applyDefaults(defaults.Log)
	c.HTTP.applyDefaults(defaults.HTTP)
	c.NATS.applyDefaults(defaults.NATS)
	c.Kafka.applyDefaults(defaults.Kafka)
	c.Bootstrap.applyDefaults(defaults.Bootstrap)
	c.Emulator.applyDefaults(defaults.Emulator)
}

// Validate checks whether the config is structurally valid.
func (c AppConfig) Validate() *problem.Problem {
	var issues []problem.ValidationIssue
	issues = append(issues, extractIssues(c.Log.Validate())...)
	issues = append(issues, extractIssues(c.HTTP.Validate())...)
	issues = append(issues, extractIssues(c.NATS.Validate())...)
	issues = append(issues, extractIssues(c.Kafka.Validate())...)
	issues = append(issues, extractIssues(c.Bootstrap.Validate())...)
	issues = append(issues, extractIssues(c.Emulator.Validate())...)

	if len(issues) == 0 {
		return nil
	}

	return validationProblem("application config is invalid", issues...)
}

type LogLevel string

const (
	LogLevelDebug LogLevel = "debug"
	LogLevelInfo  LogLevel = "info"
	LogLevelWarn  LogLevel = "warn"
	LogLevelError LogLevel = "error"
)

type LogFormat string

const (
	LogFormatJSON LogFormat = "json"
	LogFormatText LogFormat = "text"
)

// LogConfig controls structured logging output.
type LogConfig struct {
	Level  LogLevel  `json:"level"`
	Format LogFormat `json:"format"`
}

func (l *LogConfig) applyDefaults(defaults LogConfig) {
	if l.Level == "" {
		l.Level = defaults.Level
	}
	if l.Format == "" {
		l.Format = defaults.Format
	}
}

func (l LogConfig) Validate() *problem.Problem {
	var issues []problem.ValidationIssue

	switch l.Level {
	case LogLevelDebug, LogLevelInfo, LogLevelWarn, LogLevelError:
	default:
		issues = append(issues, problem.ValidationIssue{
			Field:   "log.level",
			Message: "must be one of debug, info, warn or error",
			Value:   l.Level,
		})
	}

	switch l.Format {
	case LogFormatJSON, LogFormatText:
	default:
		issues = append(issues, problem.ValidationIssue{
			Field:   "log.format",
			Message: "must be one of json or text",
			Value:   l.Format,
		})
	}

	if len(issues) == 0 {
		return nil
	}

	return validationProblem("log config is invalid", issues...)
}

// HTTPConfig controls HTTP server defaults shared by services.
type HTTPConfig struct {
	Addr            string `json:"addr"`
	ReadTimeout     string `json:"read_timeout"`
	WriteTimeout    string `json:"write_timeout"`
	IdleTimeout     string `json:"idle_timeout"`
	ShutdownTimeout string `json:"shutdown_timeout"`
}

func (h *HTTPConfig) applyDefaults(defaults HTTPConfig) {
	if strings.TrimSpace(h.Addr) == "" {
		h.Addr = defaults.Addr
	}
	if strings.TrimSpace(h.ReadTimeout) == "" {
		h.ReadTimeout = defaults.ReadTimeout
	}
	if strings.TrimSpace(h.WriteTimeout) == "" {
		h.WriteTimeout = defaults.WriteTimeout
	}
	if strings.TrimSpace(h.IdleTimeout) == "" {
		h.IdleTimeout = defaults.IdleTimeout
	}
	if strings.TrimSpace(h.ShutdownTimeout) == "" {
		h.ShutdownTimeout = defaults.ShutdownTimeout
	}
}

func (h HTTPConfig) Validate() *problem.Problem {
	var issues []problem.ValidationIssue

	if strings.TrimSpace(h.Addr) == "" {
		issues = append(issues, problem.ValidationIssue{
			Field:   "http.addr",
			Message: "must not be empty",
		})
	}

	issues = append(issues, durationIssue("http.read_timeout", h.ReadTimeout)...)
	issues = append(issues, durationIssue("http.write_timeout", h.WriteTimeout)...)
	issues = append(issues, durationIssue("http.idle_timeout", h.IdleTimeout)...)
	issues = append(issues, durationIssue("http.shutdown_timeout", h.ShutdownTimeout)...)

	if len(issues) == 0 {
		return nil
	}

	return validationProblem("http config is invalid", issues...)
}

func (h HTTPConfig) ReadTimeoutDuration() time.Duration {
	return parseDurationOrDefault(h.ReadTimeout, 5*time.Second)
}

func (h HTTPConfig) WriteTimeoutDuration() time.Duration {
	return parseDurationOrDefault(h.WriteTimeout, 10*time.Second)
}

func (h HTTPConfig) IdleTimeoutDuration() time.Duration {
	return parseDurationOrDefault(h.IdleTimeout, time.Minute)
}

func (h HTTPConfig) ShutdownTimeoutDuration() time.Duration {
	return parseDurationOrDefault(h.ShutdownTimeout, 10*time.Second)
}

func parseDurationOrDefault(raw string, fallback time.Duration) time.Duration {
	if strings.TrimSpace(raw) == "" {
		return fallback
	}
	value, err := time.ParseDuration(raw)
	if err != nil {
		return fallback
	}
	return value
}

// NATSConfig keeps transport-neutral connection metadata required by NATS-based services.
type NATSConfig struct {
	Enabled        bool   `json:"enabled"`
	URL            string `json:"url"`
	RequestTimeout string `json:"request_timeout"`
}

// JetStreamConfig preserves the previous type name while the shared package converges on transport-agnostic naming.
type JetStreamConfig = NATSConfig

func (c *NATSConfig) applyDefaults(defaults NATSConfig) {
	if strings.TrimSpace(c.RequestTimeout) == "" {
		c.RequestTimeout = defaults.RequestTimeout
	}
}

func (c NATSConfig) Validate() *problem.Problem {
	var issues []problem.ValidationIssue

	if c.Enabled && strings.TrimSpace(c.URL) == "" {
		issues = append(issues, problem.ValidationIssue{
			Field:   "nats.url",
			Message: "must not be empty when nats is enabled",
		})
	}

	issues = append(issues, durationIssue("nats.request_timeout", c.RequestTimeout)...)

	if len(issues) == 0 {
		return nil
	}

	return validationProblem("nats config is invalid", issues...)
}

func (c NATSConfig) RequestTimeoutDuration() time.Duration {
	return parseDurationOrDefault(c.RequestTimeout, 2*time.Second)
}

type KafkaConfig struct {
	Enabled       bool     `json:"enabled"`
	Brokers       []string `json:"brokers"`
	ClientID      string   `json:"client_id,omitempty"`
	ConsumerGroup string   `json:"consumer_group,omitempty"`
	DialTimeout   string   `json:"dial_timeout"`
}

func (c *KafkaConfig) applyDefaults(defaults KafkaConfig) {
	if strings.TrimSpace(c.DialTimeout) == "" {
		c.DialTimeout = defaults.DialTimeout
	}
	c.ClientID = strings.TrimSpace(c.ClientID)
	c.ConsumerGroup = strings.TrimSpace(c.ConsumerGroup)
	if len(c.Brokers) == 0 {
		return
	}
	brokers := make([]string, 0, len(c.Brokers))
	for _, broker := range c.Brokers {
		broker = strings.TrimSpace(broker)
		if broker == "" {
			continue
		}
		brokers = append(brokers, broker)
	}
	c.Brokers = brokers
}

func (c KafkaConfig) Validate() *problem.Problem {
	var issues []problem.ValidationIssue

	if c.Enabled && len(c.Brokers) == 0 {
		issues = append(issues, problem.ValidationIssue{
			Field:   "kafka.brokers",
			Message: "must contain at least one broker when kafka is enabled",
		})
	}
	issues = append(issues, durationIssue("kafka.dial_timeout", c.DialTimeout)...)

	if len(issues) == 0 {
		return nil
	}

	return validationProblem("kafka config is invalid", issues...)
}

func (c KafkaConfig) DialTimeoutDuration() time.Duration {
	return parseDurationOrDefault(c.DialTimeout, 10*time.Second)
}

type BootstrapConfig struct {
	BaseURL   string `json:"base_url"`
	ScopeKind string `json:"scope_kind,omitempty"`
	ScopeKey  string `json:"scope_key,omitempty"`
	Timeout   string `json:"timeout"`
}

func (c *BootstrapConfig) applyDefaults(defaults BootstrapConfig) {
	c.BaseURL = strings.TrimSpace(c.BaseURL)
	c.ScopeKind = strings.ToLower(strings.TrimSpace(c.ScopeKind))
	c.ScopeKey = strings.TrimSpace(c.ScopeKey)
	if c.ScopeKind == "" {
		c.ScopeKind = defaults.ScopeKind
	}
	if c.ScopeKey == "" {
		c.ScopeKey = defaults.ScopeKey
	}
	if strings.TrimSpace(c.Timeout) == "" {
		c.Timeout = defaults.Timeout
	}
}

func (c BootstrapConfig) Validate() *problem.Problem {
	var issues []problem.ValidationIssue
	issues = append(issues, durationIssue("bootstrap.timeout", c.Timeout)...)
	if c.ScopeKind == "" && c.ScopeKey != "" {
		issues = append(issues, problem.ValidationIssue{
			Field:   "bootstrap.scope_kind",
			Message: "must not be empty when scope_key is provided",
		})
	}
	if c.ScopeKey == "" && c.ScopeKind != "" && c.ScopeKind != "global" {
		issues = append(issues, problem.ValidationIssue{
			Field:   "bootstrap.scope_key",
			Message: "must not be empty when scope_kind is provided",
		})
	}

	if len(issues) == 0 {
		return nil
	}

	return validationProblem("bootstrap config is invalid", issues...)
}

func (c BootstrapConfig) TimeoutDuration() time.Duration {
	return parseDurationOrDefault(c.Timeout, 5*time.Second)
}

type EmulatorConfig struct {
	PublishInterval string `json:"publish_interval"`
}

func (c *EmulatorConfig) applyDefaults(defaults EmulatorConfig) {
	if strings.TrimSpace(c.PublishInterval) == "" {
		c.PublishInterval = defaults.PublishInterval
	}
}

func (c EmulatorConfig) Validate() *problem.Problem {
	issues := durationIssue("emulator.publish_interval", c.PublishInterval)
	if len(issues) == 0 {
		return nil
	}
	return validationProblem("emulator config is invalid", issues...)
}

func (c EmulatorConfig) PublishIntervalDuration() time.Duration {
	return parseDurationOrDefault(c.PublishInterval, 5*time.Second)
}

func durationIssue(field, raw string) []problem.ValidationIssue {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return nil
	}

	duration, err := time.ParseDuration(raw)
	if err != nil {
		return []problem.ValidationIssue{{
			Field:   field,
			Message: "must be a valid duration",
			Value:   raw,
		}}
	}

	if duration < 0 {
		return []problem.ValidationIssue{{
			Field:   field,
			Message: "must not be negative",
			Value:   raw,
		}}
	}

	return nil
}

func unexpectedJSONTokenError() error {
	return fmt.Errorf("config file contains more than one JSON document")
}
