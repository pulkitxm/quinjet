@{
    Severity     = @('Error', 'Warning')
    ExcludeRules = @(
        # The installer test harness stubs Invoke-WebRequest for the script under
        # test, which only works when the fixture paths live in the global scope.
        'PSAvoidGlobalVars'
    )
}
