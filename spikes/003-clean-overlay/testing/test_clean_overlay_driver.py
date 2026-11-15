import unittest
from run_private_clean_overlay_live_1 import parse_submission

class SubmissionParsing(unittest.TestCase):
    def test_real_trace_shape(self):
        self.assertEqual(parse_submission('LENS_SUBMIT commit=12 rect=(528, 243, 200, 200) source=(512,227) color=#0000FF'),
            {'commit':12,'rect':(528,243,200,200),'source':(512,227),'color':'#0000FF'})
    def test_unrelated_or_incomplete_line_is_not_a_submission(self):
        self.assertIsNone(parse_submission('LENS_PRESENTED commit=12 color=#0000FF'))
        self.assertIsNone(parse_submission('LENS_SUBMIT commit=12 rect=(1,2,0,3) source=(1,2) color=#000000'))

if __name__=='__main__':unittest.main()
