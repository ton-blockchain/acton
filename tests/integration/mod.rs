mod actions_tests;
mod annotations_tests;
mod basic_unit_tests;
mod build_tests;
mod check;
mod compile_tests;
mod config_tests;
mod coverage_tests;
mod diff_tests;
mod disasm_tests;
mod fixture_tests;
mod flags_tests;
mod fmt_tests;
mod incremental_tests;
mod init_tests;
mod library_tests;
mod mappings_tests;
mod new_tests;
mod println_tests;
mod reporters_tests;
mod run_tests;
mod script_tests;
mod stdlib_tests;
#[path = "test-runner/test_runner_selection_and_fail_fast_tests.rs"]
mod test_runner_selection_and_fail_fast_tests;
#[path = "test-runner/test_runner_reporting_and_coverage_tests.rs"]
mod test_runner_reporting_and_coverage_tests;
#[path = "test-runner/test_runner_ui_tests.rs"]
mod test_runner_ui_tests;
#[path = "test-runner/test_runner_debug_port_tests.rs"]
mod test_runner_debug_port_tests;
#[path = "test-runner/test_runner_cmd_agent_h_tests.rs"]
mod test_runner_cmd_agent_h_tests;
#[path = "test-runner/test_runner_fork_network_tests.rs"]
mod test_runner_fork_network_tests;
#[path = "test-runner/test_runner_mutate_tests.rs"]
mod test_runner_mutate_tests;
#[path = "test-runner/test_runner_lib_api_matchers_tests.rs"]
mod test_runner_lib_api_matchers_tests;
#[path = "test-runner/test_runner_lib_api_map_tests.rs"]
mod test_runner_lib_api_map_tests;
#[path = "test-runner/test_runner_lib_api_exit_code_tests.rs"]
mod test_runner_lib_api_exit_code_tests;
#[path = "test-runner/test_runner_lib_api_send_single_tests.rs"]
mod test_runner_lib_api_send_single_tests;
#[path = "test-runner/test_runner_lib_api_external_tests.rs"]
mod test_runner_lib_api_external_tests;
#[path = "test-runner/test_runner_lib_api_transaction_matchers_tests.rs"]
mod test_runner_lib_api_transaction_matchers_tests;
#[path = "test-runner/test_runner_lib_api_register_code_cell_tests.rs"]
mod test_runner_lib_api_register_code_cell_tests;
#[path = "test-runner/test_runner_lib_api_account_state_tests.rs"]
mod test_runner_lib_api_account_state_tests;
#[path = "test-runner/test_runner_lib_api_wallet_mode_tests.rs"]
mod test_runner_lib_api_wallet_mode_tests;
#[path = "test-runner/test_runner_lib_api_fmt_env_tests.rs"]
mod test_runner_lib_api_fmt_env_tests;
#[path = "test-runner/test_runner_stdlib_maybe_tests.rs"]
mod test_runner_stdlib_maybe_tests;
#[path = "test-runner/test_runner_stdlib_either_tests.rs"]
mod test_runner_stdlib_either_tests;
#[path = "test-runner/test_runner_stdlib_assert_tests.rs"]
mod test_runner_stdlib_assert_tests;
#[path = "test-runner/test_runner_stdlib_expect_tests.rs"]
mod test_runner_stdlib_expect_tests;
#[path = "test-runner/test_runner_stdlib_transaction_expect_tests.rs"]
mod test_runner_stdlib_transaction_expect_tests;
#[path = "test-runner/test_runner_stdlib_af_outlist_tests.rs"]
mod test_runner_stdlib_af_outlist_tests;
#[path = "test-runner/test_runner_stdlib_ag_send_tests.rs"]
mod test_runner_stdlib_ag_send_tests;
#[path = "test-runner/test_runner_stdlib_ah_balance_tests.rs"]
mod test_runner_stdlib_ah_balance_tests;
#[path = "test-runner/test_runner_stdlib_ai_config_tests.rs"]
mod test_runner_stdlib_ai_config_tests;
#[path = "test-runner/test_runner_stdlib_aj_set_tests.rs"]
mod test_runner_stdlib_aj_set_tests;
#[path = "test-runner/test_runner_stdlib_ak_load_tests.rs"]
mod test_runner_stdlib_ak_load_tests;
#[path = "test-runner/test_runner_stdlib_al_out_tests.rs"]
mod test_runner_stdlib_al_out_tests;
#[path = "test-runner/test_runner_stdlib_am_transaction_tests.rs"]
mod test_runner_stdlib_am_transaction_tests;
#[path = "test-runner/test_runner_stdlib_an_crypto_tests.rs"]
mod test_runner_stdlib_an_crypto_tests;
#[path = "test-runner/test_runner_stdlib_ao_prompt_tests.rs"]
mod test_runner_stdlib_ao_prompt_tests;
#[path = "test-runner/test_runner_stdlib_ap_select_tests.rs"]
mod test_runner_stdlib_ap_select_tests;
#[path = "test-runner/test_runner_stdlib_aq_confirm_tests.rs"]
mod test_runner_stdlib_aq_confirm_tests;
#[path = "test-runner/test_runner_stdlib_ar_println_tests.rs"]
mod test_runner_stdlib_ar_println_tests;
#[path = "test-runner/test_runner_stdlib_as_println1_tests.rs"]
mod test_runner_stdlib_as_println1_tests;
#[path = "test-runner/test_runner_stdlib_at_keeps_tests.rs"]
mod test_runner_stdlib_at_keeps_tests;
#[path = "test-runner/test_runner_stdlib_au_fs_tests.rs"]
mod test_runner_stdlib_au_fs_tests;
#[path = "test-runner/test_runner_stdlib_av_fs_tests.rs"]
mod test_runner_stdlib_av_fs_tests;
#[path = "test-runner/test_runner_stdlib_aw_build_tests.rs"]
mod test_runner_stdlib_aw_build_tests;
#[path = "test-runner/test_runner_stdlib_ax_build_tests.rs"]
mod test_runner_stdlib_ax_build_tests;
#[path = "test-runner/test_runner_stdlib_ay_crypto_tests.rs"]
mod test_runner_stdlib_ay_crypto_tests;
#[path = "test-runner/test_runner_stdlib_az_crypto_tests.rs"]
mod test_runner_stdlib_az_crypto_tests;
#[path = "test-runner/test_runner_stdlib_ba_crypto_tests.rs"]
mod test_runner_stdlib_ba_crypto_tests;
#[path = "test-runner/test_runner_stdlib_bb_global_tests.rs"]
mod test_runner_stdlib_bb_global_tests;
#[path = "test-runner/test_runner_stdlib_bc_config_tests.rs"]
mod test_runner_stdlib_bc_config_tests;
#[path = "test-runner/test_runner_stdlib_bd_config_tests.rs"]
mod test_runner_stdlib_bd_config_tests;
#[path = "test-runner/test_runner_stdlib_be_config_tests.rs"]
mod test_runner_stdlib_be_config_tests;
#[path = "test-runner/test_runner_stdlib_bf_wallet_tests.rs"]
mod test_runner_stdlib_bf_wallet_tests;
#[path = "test-runner/test_runner_stdlib_bg_run_tests.rs"]
mod test_runner_stdlib_bg_run_tests;
#[path = "test-runner/test_runner_stdlib_bh_wait_tests.rs"]
mod test_runner_stdlib_bh_wait_tests;
#[path = "test-runner/test_runner_stdlib_bi_wait_tests.rs"]
mod test_runner_stdlib_bi_wait_tests;
#[path = "test-runner/test_runner_stdlib_bj_get_tests.rs"]
mod test_runner_stdlib_bj_get_tests;
#[path = "test-runner/test_runner_stdlib_bk_network_tests.rs"]
mod test_runner_stdlib_bk_network_tests;
#[path = "test-runner/test_runner_stdlib_bl_fetch_tests.rs"]
mod test_runner_stdlib_bl_fetch_tests;
#[path = "test-runner/test_runner_stdlib_bm_load_tests.rs"]
mod test_runner_stdlib_bm_load_tests;
#[path = "test-runner/test_runner_stdlib_bn_enable_tests.rs"]
mod test_runner_stdlib_bn_enable_tests;
#[path = "test-runner/test_runner_stdlib_bo_network_tests.rs"]
mod test_runner_stdlib_bo_network_tests;
#[path = "test-runner/test_runner_stdlib_bp_env_tests.rs"]
mod test_runner_stdlib_bp_env_tests;
#[path = "test-runner/test_runner_stdlib_bq_env_tests.rs"]
mod test_runner_stdlib_bq_env_tests;
#[path = "test-runner/test_runner_stdlib_br_format5_tests.rs"]
mod test_runner_stdlib_br_format5_tests;
#[path = "test-runner/test_runner_stdlib_bs_format2_tests.rs"]
mod test_runner_stdlib_bs_format2_tests;
#[path = "test-runner/test_runner_stdlib_bt_assert_tests.rs"]
mod test_runner_stdlib_bt_assert_tests;
#[path = "test-runner/test_runner_stdlib_bu_assert_tests.rs"]
mod test_runner_stdlib_bu_assert_tests;
#[path = "test-runner/test_runner_stdlib_bv_assert_tests.rs"]
mod test_runner_stdlib_bv_assert_tests;
#[path = "test-runner/test_runner_stdlib_bw_wallet_tests.rs"]
mod test_runner_stdlib_bw_wallet_tests;
#[path = "test-runner/test_runner_stdlib_bx_expect_tests.rs"]
mod test_runner_stdlib_bx_expect_tests;
#[path = "test-runner/test_runner_stdlib_by_expect_tests.rs"]
mod test_runner_stdlib_by_expect_tests;
#[path = "test-runner/test_runner_stdlib_bz_expect_tests.rs"]
mod test_runner_stdlib_bz_expect_tests;
#[path = "test-runner/test_runner_stdlib_ca_expect_tests.rs"]
mod test_runner_stdlib_ca_expect_tests;
#[path = "test-runner/test_runner_stdlib_cb_outlist_tests.rs"]
mod test_runner_stdlib_cb_outlist_tests;
#[path = "test-runner/test_runner_stdlib_cc_outlist_tests.rs"]
mod test_runner_stdlib_cc_outlist_tests;
#[path = "test-runner/test_runner_stdlib_cd_outlist_tests.rs"]
mod test_runner_stdlib_cd_outlist_tests;
#[path = "test-runner/test_runner_stdlib_ce_to_tests.rs"]
mod test_runner_stdlib_ce_to_tests;
#[path = "test-runner/test_runner_stdlib_cf_to_tests.rs"]
mod test_runner_stdlib_cf_to_tests;
#[path = "test-runner/test_runner_stdlib_cg_to_tests.rs"]
mod test_runner_stdlib_cg_to_tests;
#[path = "test-runner/test_runner_stdlib_ch_ext_tests.rs"]
mod test_runner_stdlib_ch_ext_tests;
#[path = "test-runner/test_runner_stdlib_ci_ext_tests.rs"]
mod test_runner_stdlib_ci_ext_tests;
#[path = "test-runner/test_runner_stdlib_cj_parse_tests.rs"]
mod test_runner_stdlib_cj_parse_tests;
#[path = "test-runner/test_runner_stdlib_ck_transaction_tests.rs"]
mod test_runner_stdlib_ck_transaction_tests;
#[path = "test-runner/test_runner_stdlib_cl_set_tests.rs"]
mod test_runner_stdlib_cl_set_tests;
#[path = "test-runner/test_runner_stdlib_cm_vm_tests.rs"]
mod test_runner_stdlib_cm_vm_tests;
#[path = "test-runner/test_runner_stdlib_cn_set_tests.rs"]
mod test_runner_stdlib_cn_set_tests;
#[path = "test-runner/test_runner_stdlib_co_get_tests.rs"]
mod test_runner_stdlib_co_get_tests;
#[path = "test-runner/test_runner_stdlib_cp_net_tests.rs"]
mod test_runner_stdlib_cp_net_tests;
#[path = "test-runner/test_runner_stdlib_cq_register_tests.rs"]
mod test_runner_stdlib_cq_register_tests;
#[path = "test-runner/test_runner_stdlib_cr_register_tests.rs"]
mod test_runner_stdlib_cr_register_tests;
#[path = "test-runner/test_runner_stdlib_cs_send_tests.rs"]
mod test_runner_stdlib_cs_send_tests;
#[path = "test-runner/test_runner_stdlib_ct_ext_tests.rs"]
mod test_runner_stdlib_ct_ext_tests;
#[path = "test-runner/test_runner_stdlib_cu_out_tests.rs"]
mod test_runner_stdlib_cu_out_tests;
#[path = "test-runner/test_runner_stdlib_cv_out_tests.rs"]
mod test_runner_stdlib_cv_out_tests;
#[path = "test-runner/test_runner_stdlib_cw_out_tests.rs"]
mod test_runner_stdlib_cw_out_tests;
#[path = "test-runner/test_runner_stdlib_cx_change_tests.rs"]
mod test_runner_stdlib_cx_change_tests;
#[path = "test-runner/test_runner_stdlib_cy_out_tests.rs"]
mod test_runner_stdlib_cy_out_tests;
#[path = "test-runner/test_runner_stdlib_cz_change_tests.rs"]
mod test_runner_stdlib_cz_change_tests;
#[path = "test-runner/test_runner_stdlib_da_transaction_tests.rs"]
mod test_runner_stdlib_da_transaction_tests;
#[path = "test-runner/test_runner_stdlib_db_transaction_tests.rs"]
mod test_runner_stdlib_db_transaction_tests;
#[path = "test-runner/test_runner_stdlib_dc_transaction_tests.rs"]
mod test_runner_stdlib_dc_transaction_tests;
#[path = "test-runner/test_runner_stdlib_dd_find_tests.rs"]
mod test_runner_stdlib_dd_find_tests;
#[path = "test-runner/test_runner_stdlib_de_wait_tests.rs"]
mod test_runner_stdlib_de_wait_tests;
#[path = "test-runner/test_runner_stdlib_df_net_tests.rs"]
mod test_runner_stdlib_df_net_tests;
#[path = "test-runner/test_runner_stdlib_dh_create_tests.rs"]
mod test_runner_stdlib_dh_create_tests;
#[path = "test-runner/test_runner_stdlib_di_net_tests.rs"]
mod test_runner_stdlib_di_net_tests;
#[path = "test-runner/test_runner_stdlib_dj_vm_tests.rs"]
mod test_runner_stdlib_dj_vm_tests;
#[path = "test-runner/test_runner_stdlib_dk_vm_tests.rs"]
mod test_runner_stdlib_dk_vm_tests;
#[path = "test-runner/test_runner_stdlib_dl_vm_tests.rs"]
mod test_runner_stdlib_dl_vm_tests;
#[path = "test-runner/test_runner_stdlib_dm_config_tests.rs"]
mod test_runner_stdlib_dm_config_tests;
#[path = "test-runner/test_runner_stdlib_dn_config_tests.rs"]
mod test_runner_stdlib_dn_config_tests;
#[path = "test-runner/test_runner_stdlib_do_config_tests.rs"]
mod test_runner_stdlib_do_config_tests;
#[path = "test-runner/test_runner_stdlib_dp_config_tests.rs"]
mod test_runner_stdlib_dp_config_tests;
#[path = "test-runner/test_runner_stdlib_dq_config_tests.rs"]
mod test_runner_stdlib_dq_config_tests;
#[path = "test-runner/test_runner_stdlib_dr_config_tests.rs"]
mod test_runner_stdlib_dr_config_tests;
#[path = "test-runner/test_runner_stdlib_ds_config_tests.rs"]
mod test_runner_stdlib_ds_config_tests;
#[path = "test-runner/test_runner_stdlib_dt_config_tests.rs"]
mod test_runner_stdlib_dt_config_tests;
#[path = "test-runner/test_runner_stdlib_du_precompiled_tests.rs"]
mod test_runner_stdlib_du_precompiled_tests;
#[path = "test-runner/test_runner_stdlib_dv_vm_tests.rs"]
mod test_runner_stdlib_dv_vm_tests;
#[path = "test-runner/test_runner_stdlib_dw_crypto_tests.rs"]
mod test_runner_stdlib_dw_crypto_tests;
#[path = "test-runner/test_runner_stdlib_dx_mnemonic_tests.rs"]
mod test_runner_stdlib_dx_mnemonic_tests;
#[path = "test-runner/test_runner_stdlib_dy_crypto_tests.rs"]
mod test_runner_stdlib_dy_crypto_tests;
#[path = "test-runner/test_runner_stdlib_dz_crypto_tests.rs"]
mod test_runner_stdlib_dz_crypto_tests;
#[path = "test-runner/test_runner_stdlib_ea_format3_tests.rs"]
mod test_runner_stdlib_ea_format3_tests;
#[path = "test-runner/test_runner_stdlib_eb_format1_tests.rs"]
mod test_runner_stdlib_eb_format1_tests;
#[path = "test-runner/test_runner_stdlib_ec_expect_tests.rs"]
mod test_runner_stdlib_ec_expect_tests;
#[path = "test-runner/test_runner_stdlib_ed_map_tests.rs"]
mod test_runner_stdlib_ed_map_tests;
#[path = "test-runner/test_runner_stdlib_ee_expect_tests.rs"]
mod test_runner_stdlib_ee_expect_tests;
#[path = "test-runner/test_runner_stdlib_ef_expect_tests.rs"]
mod test_runner_stdlib_ef_expect_tests;
#[path = "test-runner/test_runner_stdlib_eg_assert_tests.rs"]
mod test_runner_stdlib_eg_assert_tests;
#[path = "test-runner/test_runner_stdlib_eh_env_tests.rs"]
mod test_runner_stdlib_eh_env_tests;
#[path = "test-runner/test_runner_stdlib_ei_crc16_tests.rs"]
mod test_runner_stdlib_ei_crc16_tests;
#[path = "test-runner/test_runner_stdlib_ej_assert_tests.rs"]
mod test_runner_stdlib_ej_assert_tests;
#[path = "test-runner/test_runner_stdlib_ek_assert_tests.rs"]
mod test_runner_stdlib_ek_assert_tests;
#[path = "test-runner/test_runner_stdlib_el_assert_tests.rs"]
mod test_runner_stdlib_el_assert_tests;
#[path = "test-runner/test_runner_stdlib_em_external_tests.rs"]
mod test_runner_stdlib_em_external_tests;
#[path = "test-runner/test_runner_stdlib_en_find_tests.rs"]
mod test_runner_stdlib_en_find_tests;
#[path = "test-runner/test_runner_stdlib_eo_transaction_tests.rs"]
mod test_runner_stdlib_eo_transaction_tests;
#[path = "test-runner/test_runner_stdlib_ep_declared_tests.rs"]
mod test_runner_stdlib_ep_declared_tests;
#[path = "test-runner/test_runner_stdlib_u_prompts_tests.rs"]
mod test_runner_stdlib_u_prompts_tests;
#[path = "test-runner/test_runner_stdlib_v_println_tests.rs"]
mod test_runner_stdlib_v_println_tests;
#[path = "test-runner/test_runner_stdlib_w_fs_tests.rs"]
mod test_runner_stdlib_w_fs_tests;
#[path = "test-runner/test_runner_stdlib_x_build_tests.rs"]
mod test_runner_stdlib_x_build_tests;
#[path = "test-runner/test_runner_stdlib_y_format1_tests.rs"]
mod test_runner_stdlib_y_format1_tests;
#[path = "test-runner/test_runner_stdlib_z_env_tests.rs"]
mod test_runner_stdlib_z_env_tests;
mod test_tests;
mod verify_tests;
mod wallet_tests;
mod wrapper_tests;
