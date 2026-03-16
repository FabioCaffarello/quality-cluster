package dataplane

import (
	"encoding/json"
	"fmt"
	"strconv"
	"strings"
	"time"

	configctlcontracts "internal/application/configctl/contracts"
	"internal/shared/problem"
)

type SyntheticScenario string

const (
	SyntheticScenarioValid               SyntheticScenario = "valid"
	SyntheticScenarioInvalidMissingField SyntheticScenario = "invalid_missing_required"
)

type SyntheticInput struct {
	Now      time.Time
	Sequence int64
	Scenario SyntheticScenario
}

type SyntheticRecord struct {
	Key      string
	Payload  json.RawMessage
	Scenario SyntheticScenario
}

func BuildSyntheticRecord(binding configctlcontracts.ActiveIngestionBindingRecord, input SyntheticInput) (SyntheticRecord, *problem.Problem) {
	if len(binding.Fields) == 0 {
		return SyntheticRecord{}, problem.New(problem.InvalidArgument, "active ingestion binding does not expose fields for emulation")
	}
	if input.Now.IsZero() {
		input.Now = time.Now().UTC()
	}
	if input.Scenario == "" {
		input.Scenario = SyntheticScenarioValid
	}

	payload := make(map[string]any, len(binding.Fields))
	for _, field := range binding.Fields {
		name := strings.TrimSpace(field.Name)
		if name == "" {
			return SyntheticRecord{}, problem.New(problem.InvalidArgument, "active ingestion binding contains a field without a name")
		}
		payload[name] = syntheticValue(binding.Binding.Name, field, input.Now, input.Sequence)
	}

	switch input.Scenario {
	case SyntheticScenarioValid:
	case SyntheticScenarioInvalidMissingField:
		fieldRemoved := false
		for _, field := range binding.Fields {
			if !field.Required {
				continue
			}
			delete(payload, strings.TrimSpace(field.Name))
			fieldRemoved = true
			break
		}
		if !fieldRemoved {
			return SyntheticRecord{}, problem.New(problem.InvalidArgument, "invalid_missing_required scenario requires at least one required field")
		}
	default:
		return SyntheticRecord{}, problem.New(problem.InvalidArgument, "synthetic scenario is unsupported")
	}

	data, err := json.Marshal(payload)
	if err != nil {
		return SyntheticRecord{}, problem.Wrap(err, problem.Internal, "encode synthetic payload")
	}

	return SyntheticRecord{
		Key:      syntheticKey(binding, input.Sequence, input.Scenario),
		Payload:  json.RawMessage(data),
		Scenario: input.Scenario,
	}, nil
}

func syntheticValue(bindingName string, field configctlcontracts.FieldRecord, now time.Time, sequence int64) any {
	switch strings.ToLower(strings.TrimSpace(field.Type)) {
	case "integer":
		return sequence
	case "number":
		return float64(sequence) + 0.5
	case "boolean":
		return sequence%2 == 0
	case "timestamp":
		return now.UTC().Format(time.RFC3339Nano)
	default:
		return fmt.Sprintf("%s-%s-%s", strings.TrimSpace(bindingName), strings.TrimSpace(field.Name), strconv.FormatInt(sequence, 10))
	}
}

func syntheticKey(binding configctlcontracts.ActiveIngestionBindingRecord, sequence int64, scenario SyntheticScenario) string {
	scopeKind := syntheticKeyToken(binding.Runtime.Scope.Kind, "global")
	scopeKey := syntheticKeyToken(binding.Runtime.Scope.Key, "default")
	bindingName := syntheticKeyToken(binding.Binding.Name, "binding")
	return fmt.Sprintf("%s-%s-%s-%d-%s", scopeKind, scopeKey, bindingName, sequence, strings.TrimSpace(string(scenario)))
}

func syntheticKeyToken(value, fallback string) string {
	value = strings.ToLower(strings.TrimSpace(value))
	if value == "" {
		return fallback
	}

	var builder strings.Builder
	for _, char := range value {
		switch {
		case char >= 'a' && char <= 'z':
			builder.WriteRune(char)
		case char >= '0' && char <= '9':
			builder.WriteRune(char)
		default:
			builder.WriteByte('-')
		}
	}

	token := strings.Trim(builder.String(), "-")
	if token == "" {
		return fallback
	}
	return token
}
