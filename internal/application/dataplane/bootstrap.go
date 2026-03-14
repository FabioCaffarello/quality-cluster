package dataplane

import (
	"sort"
	"strings"

	configctlcontracts "internal/application/configctl/contracts"
	"internal/shared/problem"
)

type BindingIndex struct {
	all     []configctlcontracts.ActiveIngestionBindingRecord
	topics  []string
	byTopic map[string][]configctlcontracts.ActiveIngestionBindingRecord
}

func NewBindingIndex(bindings []configctlcontracts.ActiveIngestionBindingRecord) (BindingIndex, *problem.Problem) {
	if len(bindings) == 0 {
		return BindingIndex{}, problem.New(problem.NotFound, "no active ingestion bindings were found")
	}

	sorted := append([]configctlcontracts.ActiveIngestionBindingRecord(nil), bindings...)
	sort.SliceStable(sorted, func(i, j int) bool {
		left := sorted[i]
		right := sorted[j]
		if left.Binding.Topic != right.Binding.Topic {
			return left.Binding.Topic < right.Binding.Topic
		}
		if left.Runtime.Scope.Kind != right.Runtime.Scope.Kind {
			return left.Runtime.Scope.Kind < right.Runtime.Scope.Kind
		}
		if left.Runtime.Scope.Key != right.Runtime.Scope.Key {
			return left.Runtime.Scope.Key < right.Runtime.Scope.Key
		}
		if left.Binding.Name != right.Binding.Name {
			return left.Binding.Name < right.Binding.Name
		}
		return left.Runtime.Config.VersionID < right.Runtime.Config.VersionID
	})

	index := BindingIndex{
		all:     make([]configctlcontracts.ActiveIngestionBindingRecord, 0, len(sorted)),
		byTopic: make(map[string][]configctlcontracts.ActiveIngestionBindingRecord),
	}
	seenBindings := make(map[string]struct{}, len(sorted))
	seenTopics := make(map[string]struct{}, len(sorted))

	for _, binding := range sorted {
		topic := strings.TrimSpace(binding.Binding.Topic)
		name := strings.TrimSpace(binding.Binding.Name)
		scopeKind := strings.TrimSpace(binding.Runtime.Scope.Kind)
		scopeKey := strings.TrimSpace(binding.Runtime.Scope.Key)
		versionID := strings.TrimSpace(binding.Runtime.Config.VersionID)

		var issues []problem.ValidationIssue
		if topic == "" {
			issues = append(issues, problem.ValidationIssue{Field: "binding.topic", Message: "must not be empty"})
		}
		if name == "" {
			issues = append(issues, problem.ValidationIssue{Field: "binding.name", Message: "must not be empty"})
		}
		if scopeKind == "" {
			issues = append(issues, problem.ValidationIssue{Field: "runtime.scope.kind", Message: "must not be empty"})
		}
		if scopeKey == "" {
			issues = append(issues, problem.ValidationIssue{Field: "runtime.scope.key", Message: "must not be empty"})
		}
		if versionID == "" {
			issues = append(issues, problem.ValidationIssue{Field: "runtime.config.version_id", Message: "must not be empty"})
		}
		if len(issues) > 0 {
			return BindingIndex{}, problem.Validation(problem.InvalidArgument, "active ingestion binding is invalid", issues...)
		}

		uniqueKey := scopeKind + "|" + scopeKey + "|" + versionID + "|" + name
		if _, exists := seenBindings[uniqueKey]; exists {
			return BindingIndex{}, problem.New(problem.Conflict, "duplicate active ingestion binding bootstrap detected")
		}
		seenBindings[uniqueKey] = struct{}{}

		index.all = append(index.all, binding)
		index.byTopic[topic] = append(index.byTopic[topic], binding)
		if _, exists := seenTopics[topic]; exists {
			continue
		}
		seenTopics[topic] = struct{}{}
		index.topics = append(index.topics, topic)
	}

	return index, nil
}

func (i BindingIndex) All() []configctlcontracts.ActiveIngestionBindingRecord {
	return append([]configctlcontracts.ActiveIngestionBindingRecord(nil), i.all...)
}

func (i BindingIndex) Topics() []string {
	return append([]string(nil), i.topics...)
}

func (i BindingIndex) BindingsForTopic(topic string) []configctlcontracts.ActiveIngestionBindingRecord {
	records := i.byTopic[strings.TrimSpace(topic)]
	return append([]configctlcontracts.ActiveIngestionBindingRecord(nil), records...)
}
