package contracts

type CreateDraftReply struct {
	Config ConfigVersionDetail `json:"config"`
}

type GetConfigReply struct {
	Config ConfigVersionDetail `json:"config"`
}

type GetActiveConfigReply struct {
	Config ConfigVersionDetail `json:"config"`
}

type ListConfigsReply struct {
	Configs []ConfigVersionSummary `json:"configs"`
}

type ListActiveIngestionBindingsReply struct {
	Bindings []ActiveIngestionBindingRecord `json:"bindings"`
}

type ValidationDiagnostic struct {
	Field   string `json:"field,omitempty"`
	Message string `json:"message"`
}

type ValidateDraftReply struct {
	Valid              bool                   `json:"valid"`
	Diagnostics        []ValidationDiagnostic `json:"diagnostics,omitempty"`
	DefinitionChecksum string                 `json:"definition_checksum,omitempty"`
}

type ValidateConfigReply struct {
	Config      ConfigVersionDetail    `json:"config"`
	Valid       bool                   `json:"valid"`
	Diagnostics []ValidationDiagnostic `json:"diagnostics,omitempty"`
}

type CompileConfigReply struct {
	Config ConfigVersionDetail `json:"config"`
}

type ActivateConfigReply struct {
	Config     ConfigVersionDetail     `json:"config"`
	Activation ActivationRecord        `json:"activation"`
	Projection RuntimeProjectionRecord `json:"projection"`
}

type DeactivateConfigReply struct {
	Config     ConfigVersionDetail `json:"config"`
	Activation ActivationRecord    `json:"activation"`
}

type ArchiveConfigReply struct {
	Config ConfigVersionDetail `json:"config"`
}

type RejectConfigReply struct {
	Config ConfigVersionDetail `json:"config"`
}
