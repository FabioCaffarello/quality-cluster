package settings

import (
	"fmt"
	"internal/shared/problem"
	"strings"
	"time"
)

type AppConfig struct {
	Log   LogConfig   `json:"log"`
	HTTP  HTTPConfig  `json:"http"`
	NATS  NATSConfig  `json:"nats"`
	Kafka KafkaConfig `json:"kafka"`
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
			RequestTimeout: "2s",
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
}

// Validate checks whether the config is structurally valid.
func (c AppConfig) Validate() *problem.Problem {
	var issues []problem.ValidationIssue
	issues = append(issues, extractIssues(c.Log.Validate())...)
	issues = append(issues, extractIssues(c.HTTP.Validate())...)
	issues = append(issues, extractIssues(c.NATS.Validate())...)
	issues = append(issues, extractIssues(c.Kafka.Validate())...)

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
	return parseDurationOrDefault(h.ReadTimeout,  5 * time.Second)
}

func (h HTTPConfig) WriteTimeoutDuration() time.Duration {
	return parseDurationOrDefault(h.WriteTimeout,  10 * time.Second)
}

func (h HTTPConfig) IdleTimeoutDuration() time.Duration {
	return parseDurationOrDefault(h.IdleTimeout,  time.Minute)
}

func (h HTTPConfig) ShutdownTimeoutDuration() time.Duration {
	return parseDurationOrDefault(h.ShutdownTimeout,  10 * time.Second)
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

// KafkaConfig keeps transport-neutral connection metadata required by Kafka-based services.
type KafkaConfig struct {
	Enabled        bool     `json:"enabled"`
	Brokers        []string `json:"brokers"`
	ClientID       string   `json:"client_id"`
	RequestTimeout string   `json:"request_timeout"`
}

func (c *KafkaConfig) applyDefaults(defaults KafkaConfig) {
	if strings.TrimSpace(c.RequestTimeout) == "" {
		c.RequestTimeout = defaults.RequestTimeout
	}
}

func (c KafkaConfig) Validate() *problem.Problem {
	var issues []problem.ValidationIssue

	if c.Enabled && len(trimmedNonEmpty(c.Brokers)) == 0 {
		issues = append(issues, problem.ValidationIssue{
			Field:   "kafka.brokers",
			Message: "must contain at least one broker when kafka is enabled",
			Value:   c.Brokers,
		})
	}

	issues = append(issues, durationIssue("kafka.request_timeout", c.RequestTimeout)...)

	if len(issues) == 0 {
		return nil
	}

	return validationProblem("kafka config is invalid", issues...)
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

func trimmedNonEmpty(values []string) []string {
	if len(values) == 0 {
		return nil
	}

	clean := make([]string, 0, len(values))
	for _, value := range values {
		value = strings.TrimSpace(value)
		if value != "" {
			clean = append(clean, value)
		}
	}
	return clean
}

func unexpectedJSONTokenError() error {
	return fmt.Errorf("config file contains more than one JSON document")
}
