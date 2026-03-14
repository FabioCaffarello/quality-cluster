package nats

type ValidatorResultsRegistry struct {
	List ControlSpec
}

func DefaultValidatorResultsRegistry() ValidatorResultsRegistry {
	return ValidatorResultsRegistry{
		List: ControlSpec{
			Subject:     "validator.results.list",
			RequestType: "validator.results.query.list",
			ReplyType:   "validator.results.reply.list",
			QueueGroup:  "validator.results",
		},
	}
}
